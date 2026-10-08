//! Pequeñas piezas de audio que no dependen del dispositivo, para poder
//! probarlas sin micrófono: el medidor de nivel y la guarda contra el eco.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Por debajo de -60 dBFS el medidor marca cero; 0 dBFS marca uno. Un rango
/// de 60 dB mueve la barra con una voz normal (-30 a -10 dBFS) y deja el
/// ruido de fondo de un micrófono abierto cerca del fondo.
const METER_FLOOR_DB: f32 = -60.0;

/// Convierte un pico de muestra (0..=32768) en un nivel de 0 a 1 en escala
/// logarítmica: es como se percibe el volumen, y una escala lineal dejaría la
/// barra casi quieta con una voz normal.
pub fn level_from_peak(peak: u16) -> f32 {
    if peak == 0 {
        return 0.0;
    }
    let db = 20.0 * (peak as f32 / 32768.0).log10();
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

pub fn peak_of(samples: &[i16]) -> u16 {
    samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0)
}

/// Junta el pico de muchos bloques de audio y entrega uno cada `interval`,
/// para no publicar un evento por cada bloque del callback de audio (decenas
/// por segundo) sino unos 15 por segundo, que sobra para una barra.
pub struct LevelMeter {
    interval: Duration,
    last_emit: Instant,
    peak: u16,
}

impl LevelMeter {
    pub fn new(interval: Duration) -> Self {
        Self { interval, last_emit: Instant::now(), peak: 0 }
    }

    /// `Some(nivel)` cuando toca publicar; `None` mientras se sigue acumulando.
    pub fn push(&mut self, samples: &[i16]) -> Option<f32> {
        self.push_at(samples, Instant::now())
    }

    fn push_at(&mut self, samples: &[i16], now: Instant) -> Option<f32> {
        self.peak = self.peak.max(peak_of(samples));
        if now.duration_since(self.last_emit) < self.interval {
            return None;
        }
        let level = level_from_peak(self.peak);
        self.peak = 0;
        self.last_emit = now;
        Some(level)
    }
}

/// Cuánto sigue el micrófono ignorado después de que el personaje deja de
/// hablar: el sonido tarda un poco en apagarse en la sala (reverberación) y
/// el reproductor avisa del final un instante antes de que se oiga el último
/// trozo.
pub const ECHO_TAIL: Duration = Duration::from_millis(400);

/// Decide cuándo el micrófono debe ignorarse porque el personaje está
/// hablando por los altavoces (si no, se oye a sí mismo, se transcribe y se
/// contesta). No es una cancelación de eco real —que resta del micrófono el
/// audio que sale por los altavoces—, sino una guarda por turnos: mientras
/// ella habla (y un instante después) no se escucha al usuario.
///
/// Lo escribe la tarea que sigue los eventos de reproducción y lo lee el
/// callback de audio, por eso son atómicos y no un candado.
pub struct EchoGuard {
    started: Instant,
    speaking: AtomicBool,
    /// Milisegundos desde `started` hasta los que el micrófono sigue ignorado.
    hold_until_ms: AtomicU64,
}

impl EchoGuard {
    pub fn new() -> Self {
        Self { started: Instant::now(), speaking: AtomicBool::new(false), hold_until_ms: AtomicU64::new(0) }
    }

    pub fn playback_started(&self) {
        self.speaking.store(true, Ordering::Relaxed);
    }

    pub fn playback_ended(&self) {
        self.playback_ended_at(Instant::now());
    }

    fn playback_ended_at(&self, now: Instant) {
        let until = now.duration_since(self.started) + ECHO_TAIL;
        self.hold_until_ms.store(until.as_millis() as u64, Ordering::Relaxed);
        self.speaking.store(false, Ordering::Relaxed);
    }

    pub fn is_holding(&self) -> bool {
        self.is_holding_at(Instant::now())
    }

    fn is_holding_at(&self, now: Instant) -> bool {
        if self.speaking.load(Ordering::Relaxed) {
            return true;
        }
        (now.duration_since(self.started).as_millis() as u64) < self.hold_until_ms.load(Ordering::Relaxed)
    }
}

impl Default for EchoGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_reads_zero_and_full_scale_reads_one() {
        assert_eq!(level_from_peak(0), 0.0);
        assert_eq!(level_from_peak(32768), 1.0);
    }

    #[test]
    fn the_scale_is_logarithmic_so_normal_speech_moves_the_bar() {
        // -30 dBFS (voz normal) cae a la mitad de la barra; una escala lineal
        // la dejaría en el 3 %.
        let speech = level_from_peak(1036);
        assert!((speech - 0.5).abs() < 0.02, "got {speech}");
        // Ruido de fondo (~-55 dBFS) queda casi en el fondo.
        assert!(level_from_peak(58) < 0.1);
        // Más fuerte siempre marca más.
        assert!(level_from_peak(2000) > level_from_peak(1000));
    }

    #[test]
    fn peak_uses_the_absolute_value_and_handles_the_most_negative_sample() {
        assert_eq!(peak_of(&[10, -300, 200]), 300);
        assert_eq!(peak_of(&[i16::MIN]), 32768);
        assert_eq!(peak_of(&[]), 0);
    }

    #[test]
    fn the_meter_reports_the_loudest_peak_since_the_last_report() {
        let mut meter = LevelMeter::new(Duration::from_millis(60));
        let t0 = meter.last_emit;
        assert_eq!(meter.push_at(&[100, -20_000], t0 + Duration::from_millis(10)), None);
        assert_eq!(meter.push_at(&[50], t0 + Duration::from_millis(30)), None);
        let level = meter.push_at(&[10], t0 + Duration::from_millis(70)).expect("toca publicar");
        assert!((level - level_from_peak(20_000)).abs() < 1e-6);
        // Y arranca de cero para el siguiente tramo.
        assert_eq!(meter.push_at(&[1], t0 + Duration::from_millis(80)), None);
        let quiet = meter.push_at(&[1], t0 + Duration::from_millis(140)).expect("toca publicar");
        assert!(quiet < 0.01);
    }

    #[test]
    fn the_guard_holds_while_speaking_and_for_a_tail_afterwards() {
        let guard = EchoGuard::new();
        let t0 = guard.started;
        assert!(!guard.is_holding_at(t0 + Duration::from_millis(5)));

        guard.playback_started();
        assert!(guard.is_holding_at(t0 + Duration::from_secs(30)), "mientras habla, siempre");

        let ended = t0 + Duration::from_secs(30);
        guard.playback_ended_at(ended);
        assert!(guard.is_holding_at(ended + Duration::from_millis(399)));
        assert!(!guard.is_holding_at(ended + Duration::from_millis(401)));
    }

    #[test]
    fn a_new_reply_right_after_the_tail_starts_holding_again() {
        let guard = EchoGuard::new();
        let t0 = guard.started;
        guard.playback_ended_at(t0 + Duration::from_secs(1));
        assert!(!guard.is_holding_at(t0 + Duration::from_secs(2)));
        guard.playback_started();
        assert!(guard.is_holding_at(t0 + Duration::from_secs(2)));
    }
}
