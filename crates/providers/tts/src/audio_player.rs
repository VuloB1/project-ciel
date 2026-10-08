use std::io::Cursor;
use std::sync::mpsc;
use std::time::Duration;

use aivt_eventbus::{AppEvent, EventBus};

/// How often the dedicated thread checks for a new command while a clip is
/// playing — the ceiling on how long `stop()`/a superseding `play()` can
/// take to actually interrupt audio. Low enough to feel instant (Fase 6's
/// barge-in depends on this), high enough not to matter for CPU usage.
const INTERRUPT_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// One thing the dedicated audio thread should do. Carries its own
/// `profile_id`/`message_id` so the thread can publish correctly-addressed
/// `AppEvent::VoicePlayback*` events without bouncing back to the caller.
enum AudioCommand {
    Play { bytes: Vec<u8>, profile_id: String, message_id: Option<String> },
    Stop,
    /// Cambia el dispositivo de salida (`None` = el predeterminado del sistema).
    SetDevice(Option<String>),
}

/// Nombres de los dispositivos de salida, para el selector de Configuración.
/// Se usa el nombre como identificador (igual que con los micrófonos): es lo
/// que muestra el sistema y suficiente para unos pocos dispositivos distintos.
#[allow(deprecated)] // `DeviceTrait::name`: ver el comentario de `list_input_devices` en `aivt_service_call`.
pub fn list_output_devices() -> Result<Vec<String>, String> {
    use rodio::cpal::traits::HostTrait;
    let devices = rodio::cpal::default_host().output_devices().map_err(|err| err.to_string())?;
    Ok(devices.filter_map(|device| rodio::DeviceTrait::name(&device).ok()).collect())
}

/// Abre el dispositivo pedido; si ya no existe (desenchufado desde que se
/// eligió) o falla, cae al predeterminado en vez de dejar la app sin sonido.
#[allow(deprecated)]
fn open_sink(device_name: Option<&str>) -> Result<rodio::stream::MixerDeviceSink, String> {
    use rodio::cpal::traits::HostTrait;
    if let Some(wanted) = device_name {
        let found = rodio::cpal::default_host()
            .output_devices()
            .ok()
            .and_then(|mut devices| devices.find(|device| rodio::DeviceTrait::name(device).ok().as_deref() == Some(wanted)));
        match found {
            Some(device) => match rodio::DeviceSinkBuilder::from_device(device).and_then(|builder| builder.open_stream()) {
                Ok(sink) => return Ok(sink),
                Err(err) => {
                    tracing::warn!(target: "system", "voz: no se pudo abrir la salida \"{wanted}\" ({err}), se usa la predeterminada");
                }
            },
            None => {
                tracing::warn!(target: "system", "voz: la salida \"{wanted}\" ya no está disponible, se usa la predeterminada");
            }
        }
    }
    rodio::DeviceSinkBuilder::open_default_sink().map_err(|err| err.to_string())
}

/// Plays HTTP-backend audio bytes through the OS's default output device.
///
/// `rodio`/`cpal` need their stream object to live on one dedicated thread
/// (the underlying platform audio APIs are not reliably safe to move across
/// threads — the same class of constraint that made `NativeTtsProvider`
/// keep its `tts::Tts` behind a single owned `Mutex` rather than passing it
/// around). Rather than gamble on `cpal::Stream`'s auto-trait status, this
/// opens the device once on a thread that never leaves, and every other
/// part of the app talks to it only through this channel.
pub struct AudioPlayer {
    tx: mpsc::Sender<AudioCommand>,
}

impl AudioPlayer {
    /// Opens the default output device on a new dedicated thread. Returns
    /// `Err` only if the device genuinely can't be opened (no audio
    /// hardware, driver failure) — the thread itself never panics on a bad
    /// `speak` call, it reports the failure as `VoicePlaybackError` instead.
    pub fn spawn(event_bus: EventBus) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<AudioCommand>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        std::thread::spawn(move || {
            let mut _handle = match open_sink(None) {
                Ok(handle) => handle,
                Err(err) => {
                    let _ = ready_tx.send(Err(err));
                    return;
                }
            };
            let mut player = rodio::Player::connect_new(_handle.mixer());
            let _ = ready_tx.send(Ok(()));

            // Which clip (if any) is currently loaded in `player`, so a
            // `VoicePlaybackFinished` can be published for it however
            // playback ends — naturally, or cut short by `Stop`/a
            // superseding `Play`. Tracked here instead of relying on
            // `Player::sleep_until_end()` returning, because that call
            // blocks this whole thread until the clip finishes on its own
            // — any `Stop` sent meanwhile just sits unconsumed in `rx`
            // until then, which is a no-op by that point. That silently
            // broke `AudioPlayer::stop()` for the HTTP backend (native TTS
            // has its own, unaffected `stop()` — see `native.rs`) and is
            // the reason Fase 6's barge-in ("interrupt the character mid-
            // reply") needs this fixed first: it depends on `stop()`
            // actually cutting audio within roughly `INTERRUPT_POLL_INTERVAL`,
            // not "whenever the current clip happens to end".
            let mut now_playing: Option<(String, Option<String>)> = None;

            loop {
                let received = if now_playing.is_some() {
                    match rx.recv_timeout(INTERRUPT_POLL_INTERVAL) {
                        Ok(command) => Some(command),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match rx.recv() {
                        Ok(command) => Some(command),
                        Err(_) => break,
                    }
                };

                if let Some(command) = received {
                    match command {
                        AudioCommand::SetDevice(name) => {
                            // Lo que suena se corta (no se puede mover a medias
                            // a otro dispositivo) y se avisa como terminado.
                            player.stop();
                            if let Some((profile_id, message_id)) = now_playing.take() {
                                event_bus.publish(AppEvent::VoicePlaybackFinished { profile_id, message_id });
                            }
                            match open_sink(name.as_deref()) {
                                Ok(new_handle) => {
                                    player = rodio::Player::connect_new(new_handle.mixer());
                                    _handle = new_handle; // mantiene vivo el dispositivo
                                }
                                Err(err) => tracing::warn!(target: "system", "voz: no se pudo cambiar la salida de audio: {err}"),
                            }
                        }
                        AudioCommand::Stop => {
                            player.stop();
                            if let Some((profile_id, message_id)) = now_playing.take() {
                                event_bus.publish(AppEvent::VoicePlaybackFinished { profile_id, message_id });
                            }
                        }
                        AudioCommand::Play { bytes, profile_id, message_id } => {
                            // A new `Play` always supersedes whatever's still
                            // queued — matches the "interrupt" semantics
                            // `NativeTtsProvider` gets from `tts::speak(_, true)`.
                            player.stop();
                            if let Some((prev_profile_id, prev_message_id)) = now_playing.take() {
                                event_bus.publish(AppEvent::VoicePlaybackFinished {
                                    profile_id: prev_profile_id,
                                    message_id: prev_message_id,
                                });
                            }
                            match rodio::Decoder::new(Cursor::new(bytes)) {
                                Ok(source) => {
                                    event_bus.publish(AppEvent::VoicePlaybackStarted {
                                        profile_id: profile_id.clone(),
                                        message_id: message_id.clone(),
                                    });
                                    player.append(source);
                                    now_playing = Some((profile_id, message_id));
                                }
                                Err(err) => {
                                    event_bus.publish(AppEvent::VoicePlaybackError {
                                        profile_id,
                                        message_id,
                                        message: format!("audio inválido recibido del servidor de voz: {err}"),
                                    });
                                }
                            }
                        }
                    }
                }

                if now_playing.is_some() && player.empty() {
                    if let Some((profile_id, message_id)) = now_playing.take() {
                        event_bus.publish(AppEvent::VoicePlaybackFinished { profile_id, message_id });
                    }
                }
            }
        });

        ready_rx.recv().map_err(|_| "el hilo de audio terminó antes de inicializarse".to_string())??;
        Ok(Self { tx })
    }

    /// A player whose thread never started — every `play`/`stop` call fails
    /// cleanly instead of the app refusing to start when no audio output
    /// device could be opened at all (see `AivtTtsProvider::new`).
    pub fn dead() -> Self {
        let (tx, _rx) = mpsc::channel();
        Self { tx }
    }

    pub fn play(&self, bytes: Vec<u8>, profile_id: String, message_id: Option<String>) -> Result<(), String> {
        self.tx
            .send(AudioCommand::Play { bytes, profile_id, message_id })
            .map_err(|_| "el hilo de audio ya no está disponible".to_string())
    }

    pub fn set_device(&self, name: Option<String>) -> Result<(), String> {
        self.tx.send(AudioCommand::SetDevice(name)).map_err(|_| "el hilo de audio ya no está disponible".to_string())
    }

    pub fn stop(&self) -> Result<(), String> {
        self.tx.send(AudioCommand::Stop).map_err(|_| "el hilo de audio ya no está disponible".to_string())
    }
}
