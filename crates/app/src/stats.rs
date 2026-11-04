//! Métricas del callback de audio. Todo son atómicos: el hilo de audio solo
//! escribe y el hilo principal lee, sin locks ni reservas de memoria.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Duration;

#[derive(Default)]
pub struct AudioStats {
    callbacks: AtomicU64,
    total_frames: AtomicU64,
    last_frames: AtomicU64,
    busy_ns: AtomicU64,
    /// Máximo desde el último informe.
    interval_max_ns: AtomicU64,
    /// Máximo de toda la ejecución.
    peak_ns: AtomicU64,
    /// Callbacks que tardaron más que lo que dura su buffer.
    misses: AtomicU64,
    stream_errors: AtomicU64,
}

pub struct Report {
    pub callbacks: u64,
    pub frames_per_callback: u64,
    /// Fracción media del tiempo de buffer usada por el callback (1.0 = 100 %).
    pub avg_load: f64,
    /// Peor callback desde el informe anterior, como fracción del tiempo de buffer.
    pub interval_max_load: f64,
    /// Peor callback de toda la ejecución, como fracción del tiempo de buffer.
    pub peak_load: f64,
    pub misses: u64,
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

    /// Instantánea de las métricas; reinicia el máximo del intervalo.
    pub fn take_report(&self, sample_rate: u32) -> Report {
        let frames = self.last_frames.load(Relaxed).max(1);
        let buffer_ns = (frames * 1_000_000_000 / sample_rate as u64).max(1) as f64;
        let played_ns = (self.total_frames.load(Relaxed) * 1_000_000_000 / sample_rate as u64).max(1);
        Report {
            callbacks: self.callbacks.load(Relaxed),
            frames_per_callback: frames,
            avg_load: self.busy_ns.load(Relaxed) as f64 / played_ns as f64,
            interval_max_load: self.interval_max_ns.swap(0, Relaxed) as f64 / buffer_ns,
            peak_load: self.peak_ns.load(Relaxed) as f64 / buffer_ns,
            misses: self.misses.load(Relaxed),
            stream_errors: self.stream_errors.load(Relaxed),
        }
    }
}
