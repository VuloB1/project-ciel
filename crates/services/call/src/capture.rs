use std::sync::mpsc as std_mpsc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};

use crate::error::CallError;

/// Every downstream consumer (VAD, STT) works in mono 16kHz i16 PCM —
/// `webrtc-vad`'s most common rate and Whisper's native input rate, so
/// there's exactly one conversion (here) instead of two.
pub const CAPTURE_SAMPLE_RATE: u32 = 16000;

/// Owns the input `cpal::Stream` on a dedicated OS thread — platform audio
/// handles aren't reliably `Send` (same justification as
/// `aivt_provider_tts::audio_player::AudioPlayer::spawn`). Always captures
/// whatever format the default input device natively reports (via
/// `default_input_config`) and converts to mono 16kHz i16 PCM in software
/// (downmix + linear-interpolation resample, see `Resampler`) rather than
/// asking the device for that format directly — most devices/drivers only
/// support their own native WASAPI mix format (commonly f32 at
/// 44.1/48kHz), so a specific-format request is a request that can simply
/// fail; converting ourselves is the reliable path.
pub struct MicCapture {
    _stream_thread: std::thread::JoinHandle<()>,
    stop_tx: std_mpsc::Sender<()>,
}

/// Lists every input-capable device's name, for a settings picker — the
/// first entry a user with more than one microphone actually wants isn't
/// necessarily whatever Windows currently calls "default." Device *names*
/// are used as the stable-enough identifier (matches how they're picked
/// back out in `MicCapture::start`). `DeviceTrait::name` is deprecated in
/// favor of `id()`/`description()` (a more granular API distinguishing a
/// stable cross-reboot id from display info) — not adopted here since it'd
/// mean storing an opaque id in settings *and* a separate lookup for the
/// human-readable label the UI needs; plain names remain accurate for the
/// common case of a handful of distinctly-named devices.
#[allow(deprecated)]
pub fn list_input_devices() -> Result<Vec<String>, CallError> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|err| CallError::MicrophoneUnavailable(err.to_string()))?;
    Ok(devices.filter_map(|device| device.name().ok()).collect())
}

impl MicCapture {
    /// `on_samples` is called from the dedicated audio thread with mono
    /// 16kHz i16 PCM chunks as they become available — keep it fast and
    /// non-blocking, same contract as any realtime audio callback (it
    /// should just push into a channel, never block on I/O or a lock held
    /// elsewhere).
    ///
    /// `device_name`: `None` (or a name that no longer matches any
    /// connected device — e.g. unplugged since it was picked) falls back
    /// to the system default input device, logged as a warning rather than
    /// failing outright.
    #[allow(deprecated)] // See `list_input_devices`'s doc comment on `DeviceTrait::name`.
    pub fn start(device_name: Option<String>, mut on_samples: impl FnMut(&[i16]) + Send + 'static) -> Result<Self, CallError> {
        let (ready_tx, ready_rx) = std_mpsc::channel::<Result<(), String>>();
        let (stop_tx, stop_rx) = std_mpsc::channel::<()>();

        let stream_thread = std::thread::spawn(move || {
            let host = cpal::default_host();
            let requested = device_name.as_deref().and_then(|wanted| {
                let matched = host.input_devices().ok()?.find(|device| device.name().ok().as_deref() == Some(wanted));
                if matched.is_none() {
                    tracing::warn!(target: "system", "call: el micrófono \"{wanted}\" ya no está disponible, se usa el predeterminado");
                }
                matched
            });
            let device = match requested.or_else(|| host.default_input_device()) {
                Some(device) => device,
                None => {
                    let _ = ready_tx.send(Err("no se encontró un dispositivo de micrófono".to_string()));
                    return;
                }
            };
            let config = match device.default_input_config() {
                Ok(config) => config,
                Err(err) => {
                    let _ = ready_tx.send(Err(err.to_string()));
                    return;
                }
            };

            let sample_format = config.sample_format();
            let channels = config.channels() as usize;
            let source_rate = config.sample_rate();
            let stream_config: StreamConfig = config.into();

            let err_fn = |err| tracing::warn!(target: "system", "call: error de captura de audio: {err}");

            let stream_result = match sample_format {
                SampleFormat::F32 => {
                    let mut resampler = Resampler::new(source_rate, CAPTURE_SAMPLE_RATE, channels);
                    device.build_input_stream(
                        &stream_config,
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            let pcm = resampler.process(&downmix_f32(data, resampler.channels));
                            on_samples(&pcm);
                        },
                        err_fn,
                        None,
                    )
                }
                SampleFormat::I16 => {
                    let mut resampler = Resampler::new(source_rate, CAPTURE_SAMPLE_RATE, channels);
                    device.build_input_stream(
                        &stream_config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            let mono: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                            let pcm = resampler.process(&downmix_f32(&mono, resampler.channels));
                            on_samples(&pcm);
                        },
                        err_fn,
                        None,
                    )
                }
                SampleFormat::U16 => {
                    let mut resampler = Resampler::new(source_rate, CAPTURE_SAMPLE_RATE, channels);
                    device.build_input_stream(
                        &stream_config,
                        move |data: &[u16], _: &cpal::InputCallbackInfo| {
                            // u16 PCM centers silence at 32768 (`1 << 15`).
                            let mono: Vec<f32> = data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).collect();
                            let pcm = resampler.process(&downmix_f32(&mono, resampler.channels));
                            on_samples(&pcm);
                        },
                        err_fn,
                        None,
                    )
                }
                other => {
                    let _ = ready_tx.send(Err(format!("formato de audio no soportado: {other:?}")));
                    return;
                }
            };

            let stream = match stream_result {
                Ok(stream) => stream,
                Err(err) => {
                    let _ = ready_tx.send(Err(err.to_string()));
                    return;
                }
            };

            if let Err(err) = stream.play() {
                let _ = ready_tx.send(Err(err.to_string()));
                return;
            }

            let _ = ready_tx.send(Ok(()));

            // `stream` must stay alive for as long as the call is active —
            // dropping it stops capture. Block this thread until told to.
            let _ = stop_rx.recv();
        });

        ready_rx
            .recv()
            .map_err(|_| CallError::MicrophoneUnavailable("el hilo de captura terminó antes de inicializarse".to_string()))?
            .map_err(CallError::MicrophoneUnavailable)?;

        Ok(Self { _stream_thread: stream_thread, stop_tx })
    }
}

impl Drop for MicCapture {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
    }
}

/// Averages interleaved multi-channel samples down to mono. A no-op copy
/// when `channels == 1`.
fn downmix_f32(data: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return data.to_vec();
    }
    data.chunks(channels).map(|frame| frame.iter().sum::<f32>() / frame.len() as f32).collect()
}

/// Linear-interpolation resampler with state carried across calls, so
/// consecutive chunks from the audio callback don't click at their
/// boundary. `carry` always holds exactly the not-yet-fully-consumed tail
/// of mono samples; `cursor` is always relative to `carry`'s start, so
/// there's never a negative-index case to special-case.
struct Resampler {
    source_rate: u32,
    target_rate: u32,
    channels: usize,
    carry: Vec<f32>,
    cursor: f64,
}

impl Resampler {
    fn new(source_rate: u32, target_rate: u32, channels: usize) -> Self {
        Self { source_rate, target_rate, channels: channels.max(1), carry: Vec::new(), cursor: 0.0 }
    }

    /// `mono` is already downmixed — this only resamples.
    fn process(&mut self, mono: &[f32]) -> Vec<i16> {
        if self.source_rate == self.target_rate {
            return mono.iter().map(|&s| to_i16(s)).collect();
        }

        let mut buffer = std::mem::take(&mut self.carry);
        buffer.extend_from_slice(mono);

        let ratio = self.source_rate as f64 / self.target_rate as f64;
        let mut output = Vec::new();

        while self.cursor + 1.0 < buffer.len() as f64 {
            let idx = self.cursor.floor() as usize;
            let frac = (self.cursor - idx as f64) as f32;
            let sample = buffer[idx] + (buffer[idx + 1] - buffer[idx]) * frac;
            output.push(to_i16(sample));
            self.cursor += ratio;
        }

        let consumed = (self.cursor.floor() as usize).min(buffer.len());
        self.cursor -= consumed as f64;
        self.carry = buffer[consumed..].to_vec();

        output
    }
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_f32_passes_mono_through_unchanged() {
        let mono = downmix_f32(&[0.1, -0.2, 0.3], 1);
        assert_eq!(mono, vec![0.1, -0.2, 0.3]);
    }

    #[test]
    fn downmix_f32_averages_stereo_channels() {
        let mono = downmix_f32(&[1.0, 0.0, -1.0, 0.0], 2);
        assert_eq!(mono, vec![0.5, -0.5]);
    }

    #[test]
    fn resampler_at_the_same_rate_is_a_passthrough() {
        let mut resampler = Resampler::new(16000, 16000, 1);
        let out = resampler.process(&[0.5, -0.5, 0.25]);
        assert_eq!(out, vec![to_i16(0.5), to_i16(-0.5), to_i16(0.25)]);
    }

    #[test]
    fn resampler_downsamples_to_roughly_the_expected_length() {
        // 48000 -> 16000 is a 3:1 ratio.
        let mut resampler = Resampler::new(48000, 16000, 1);
        let input: Vec<f32> = (0..4800).map(|i| (i as f32 / 100.0).sin()).collect();
        let out = resampler.process(&input);
        let expected = input.len() / 3;
        assert!((out.len() as i64 - expected as i64).abs() <= 2, "got {} expected ~{expected}", out.len());
    }

    #[test]
    fn resampler_stays_continuous_across_chunk_boundaries() {
        // A constant-value signal split across two calls must resample to
        // (approximately) the same constant value throughout — any
        // discontinuity in the carry/cursor bookkeeping would show up as a
        // spike near the boundary.
        let mut resampler = Resampler::new(48000, 16000, 1);
        let first_chunk = vec![0.5; 300];
        let second_chunk = vec![0.5; 300];

        let mut all_out = resampler.process(&first_chunk);
        all_out.extend(resampler.process(&second_chunk));

        assert!(!all_out.is_empty());
        for sample in &all_out {
            assert!((*sample as i32 - to_i16(0.5) as i32).abs() <= 1, "sample {sample} drifted from the constant input");
        }
    }

    #[test]
    fn resampler_never_reads_out_of_bounds_on_tiny_chunks() {
        // Feed pathologically small chunks (smaller than one output step)
        // to make sure the carry/cursor bookkeeping never indexes past the
        // end of `buffer` — this would panic if it did.
        let mut resampler = Resampler::new(48000, 16000, 1);
        for _ in 0..50 {
            let _ = resampler.process(&[0.1]);
        }
    }
}
