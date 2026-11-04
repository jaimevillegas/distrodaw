//! Métricas del callback de audio. Todo son atómicos: el hilo de audio solo
//! escribe y el hilo principal lee, sin locks ni reservas de memoria.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::Duration;

#[derive(Default)]
pub struct AudioStats {
    callbacks: AtomicU64,
    total_frames: AtomicU64,
    last_frames: AtomicU64,
    busy_ns: AtomicU64,
    /// Valores de `busy_ns` y `total_frames` en el informe anterior.
    prev_busy_ns: AtomicU64,
    prev_frames: AtomicU64,
    /// Máximo desde el último informe.
    interval_max_ns: AtomicU64,
    /// Máximo de toda la ejecución.
    peak_ns: AtomicU64,
    /// Callbacks que tardaron más que lo que dura su buffer.
    misses: AtomicU64,
    /// Subdesbordamientos que notifica el controlador de audio.
    xruns: AtomicU64,
    /// Otros errores del stream.
    stream_errors: AtomicU64,
    /// El dispositivo de audio desapareció (p. ej. se desconectó el USB).
    device_lost: AtomicBool,
}

pub struct Report {
    pub callbacks: u64,
    pub frames_per_callback: u64,
    /// Fracción media del tiempo de buffer usada por el callback (1.0 = 100 %).
    pub avg_load: f64,
    /// Igual que `avg_load` pero solo desde el informe anterior; refleja la carga actual.
    pub interval_avg_load: f64,
    /// Peor callback desde el informe anterior, como fracción del tiempo de buffer.
    pub interval_max_load: f64,
    /// Peor callback de toda la ejecución, como fracción del tiempo de buffer.
    pub peak_load: f64,
    /// Peor callback de toda la ejecución, en milisegundos. Si supera la
    /// duración del buffer del dispositivo, hubo corte seguro.
    pub peak_ms: f64,
    pub misses: u64,
    pub xruns: u64,
    pub stream_errors: u64,
}

impl AudioStats {
    /// Se llama al final de cada callback con lo que tardó en rellenar `frames`.
    pub fn record(&self, frames: usize, busy: Duration, sample_rate: u32) {
        let frames = frames as u64;
        let busy_ns = busy.as_nanos() as u64;
        let deadline_ns = frames * 1_000_000_000 / sample_rate as u64;
        self.callbacks.fetch_add(1, Relaxed);
        self.total_frames.fetch_add(frames, Relaxed);
        self.last_frames.store(frames, Relaxed);
        self.busy_ns.fetch_add(busy_ns, Relaxed);
        self.interval_max_ns.fetch_max(busy_ns, Relaxed);
        self.peak_ns.fetch_max(busy_ns, Relaxed);
        if busy_ns > deadline_ns {
            self.misses.fetch_add(1, Relaxed);
        }
    }

    pub fn record_stream_error(&self) {
        self.stream_errors.fetch_add(1, Relaxed);
    }

    pub fn record_xrun(&self) {
        self.xruns.fetch_add(1, Relaxed);
    }

    pub fn mark_device_lost(&self) {
        self.device_lost.store(true, Relaxed);
    }

    /// `true` una sola vez por cada pérdida del dispositivo.
    pub fn take_device_lost(&self) -> bool {
        self.device_lost.swap(false, Relaxed)
    }

    /// Instantánea de las métricas; reinicia el máximo del intervalo.
    pub fn take_report(&self, sample_rate: u32) -> Report {
        let frames = self.last_frames.load(Relaxed).max(1);
        let buffer_ns = (frames * 1_000_000_000 / sample_rate as u64).max(1) as f64;
        let played_ns = (self.total_frames.load(Relaxed) * 1_000_000_000 / sample_rate as u64).max(1);
        let busy_ns = self.busy_ns.load(Relaxed);
        let total_frames = self.total_frames.load(Relaxed);
        let interval_busy = busy_ns.saturating_sub(self.prev_busy_ns.swap(busy_ns, Relaxed));
        let interval_frames = total_frames.saturating_sub(self.prev_frames.swap(total_frames, Relaxed));
        let interval_played_ns = (interval_frames * 1_000_000_000 / sample_rate as u64).max(1);
        Report {
            interval_avg_load: interval_busy as f64 / interval_played_ns as f64,
            callbacks: self.callbacks.load(Relaxed),
            frames_per_callback: frames,
            avg_load: self.busy_ns.load(Relaxed) as f64 / played_ns as f64,
            interval_max_load: self.interval_max_ns.swap(0, Relaxed) as f64 / buffer_ns,
            peak_load: self.peak_ns.load(Relaxed) as f64 / buffer_ns,
            peak_ms: self.peak_ns.load(Relaxed) as f64 / 1e6,
            misses: self.misses.load(Relaxed),
            xruns: self.xruns.load(Relaxed),
            stream_errors: self.stream_errors.load(Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    #[test]
    fn interval_load_follows_recent_callbacks_not_the_whole_run() {
        let stats = AudioStats::default();
        // 1000 frames (20,83 ms) usando 10 ms: carga ≈ 48 %.
        stats.record(1000, Duration::from_millis(10), SR);
        let first = stats.take_report(SR);
        assert!((first.interval_avg_load - 0.48).abs() < 0.01);

        // Un intervalo sin trabajo útil baja la carga, aunque la media total siga alta.
        stats.record(1000, Duration::from_micros(100), SR);
        let second = stats.take_report(SR);
        assert!(second.interval_avg_load < 0.01);
        assert!(second.avg_load > second.interval_avg_load);
    }

    #[test]
    fn counts_deadline_misses() {
        let stats = AudioStats::default();
        stats.record(480, Duration::from_millis(20), SR); // buffer de 10 ms
        stats.record(480, Duration::from_millis(1), SR);
        let r = stats.take_report(SR);
        assert_eq!(r.misses, 1);
        assert_eq!(r.callbacks, 2);
    }

    #[test]
    fn device_loss_is_reported_once() {
        let stats = AudioStats::default();
        assert!(!stats.take_device_lost());
        stats.mark_device_lost();
        stats.mark_device_lost();
        assert!(stats.take_device_lost());
        assert!(!stats.take_device_lost());
    }

    #[test]
    fn xruns_and_other_errors_are_counted_apart() {
        let stats = AudioStats::default();
        stats.record_xrun();
        stats.record_xrun();
        stats.record_stream_error();
        let r = stats.take_report(SR);
        assert_eq!((r.xruns, r.stream_errors), (2, 1));
    }
}
