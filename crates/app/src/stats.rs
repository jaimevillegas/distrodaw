//! Métricas del callback de audio. Todo son atómicos: el hilo de audio solo
//! escribe y el hilo principal lee, sin locks ni reservas de memoria.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

/// Un callback se considera «retrasado» si llega después de más de este
/// múltiplo de su periodo: el hilo de audio no se ejecutó a tiempo.
const LATE_FACTOR: u64 = 2;

/// Estado de la prioridad de tiempo real del hilo de audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Realtime {
    /// Aún no ha arrancado el hilo de audio.
    Pending,
    /// Desactivado con `--rt-priority 0`.
    Off,
    /// Activo con esta prioridad `SCHED_FIFO`.
    Active(i32),
    /// El sistema lo denegó (código `errno`, normalmente `EPERM`).
    Denied(i32),
}

/// Codificación en un atómico: 0 pendiente, `i32::MIN` desactivado, >0 activo, <0 `-errno`.
const RT_OFF: i32 = i32::MIN;

/// Origen de tiempos para medir los intervalos entre callbacks.
struct Base(Instant);

impl Default for Base {
    fn default() -> Self {
        Base(Instant::now())
    }
}

#[derive(Default)]
pub struct AudioStats {
    base: Base,
    /// Instante de inicio del último callback, en ns desde `base` más 1 (0 = ninguno).
    last_start_ns: AtomicU64,
    /// Intervalo más largo entre dos callbacks seguidos (ns): toda la ejecución y desde el último informe.
    max_gap_ns: AtomicU64,
    interval_max_gap_ns: AtomicU64,
    /// Callbacks que llegaron con más de `LATE_FACTOR` periodos de retraso.
    late: AtomicU64,
    realtime: AtomicI32,
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
    /// Callbacks que llegaron tarde: el hilo de audio tardó en ejecutarse.
    pub late: u64,
    /// Mayor intervalo entre callbacks de toda la ejecución, en milisegundos.
    pub max_gap_ms: f64,
    /// Mayor intervalo entre callbacks desde el informe anterior, en milisegundos.
    pub interval_max_gap_ms: f64,
    pub realtime: Realtime,
}

impl AudioStats {
    /// Se llama al final de cada callback. `start` es cuándo empezó y `busy`
    /// lo que tardó en rellenar `frames`.
    pub fn record(&self, frames: usize, start: Instant, busy: Duration, sample_rate: u32) {
        let frames = frames as u64;
        let busy_ns = busy.as_nanos() as u64;
        let deadline_ns = frames * 1_000_000_000 / sample_rate as u64;

        let now_ns = start.saturating_duration_since(self.base.0).as_nanos() as u64 + 1;
        let previous = self.last_start_ns.swap(now_ns, Relaxed);
        if previous != 0 {
            let gap_ns = now_ns.saturating_sub(previous);
            self.max_gap_ns.fetch_max(gap_ns, Relaxed);
            self.interval_max_gap_ns.fetch_max(gap_ns, Relaxed);
            if gap_ns > LATE_FACTOR * deadline_ns {
                self.late.fetch_add(1, Relaxed);
            }
        }
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

    /// El hilo de audio anota si consiguió la prioridad de tiempo real.
    pub fn record_realtime(&self, priority: i32, outcome: &std::io::Result<()>) {
        let value = match outcome {
            _ if priority <= 0 => RT_OFF,
            Ok(()) => priority,
            Err(e) => -e.raw_os_error().unwrap_or(1).max(1),
        };
        self.realtime.store(value, Relaxed);
    }

    pub fn realtime(&self) -> Realtime {
        match self.realtime.load(Relaxed) {
            0 => Realtime::Pending,
            RT_OFF => Realtime::Off,
            p if p > 0 => Realtime::Active(p),
            e => Realtime::Denied(-e),
        }
    }

    pub fn record_xrun(&self) {
        self.xruns.fetch_add(1, Relaxed);
    }

    /// Olvida el instante del último callback. Hay que llamarlo al abrir un
    /// stream nuevo para no contar como retraso el tiempo que estuvo cerrado.
    pub fn reset_gap(&self) {
        self.last_start_ns.store(0, Relaxed);
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
            late: self.late.load(Relaxed),
            max_gap_ms: self.max_gap_ns.load(Relaxed) as f64 / 1e6,
            interval_max_gap_ms: self.interval_max_gap_ns.swap(0, Relaxed) as f64 / 1e6,
            realtime: self.realtime(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn rec(stats: &AudioStats, frames: usize, busy: Duration) {
        stats.record(frames, Instant::now(), busy, SR);
    }

    /// Callback que empezó `at_ms` milisegundos después del origen de la estadística.
    fn rec_at(stats: &AudioStats, at_ms: u64, frames: usize) {
        let start = stats.base.0 + Duration::from_millis(at_ms);
        stats.record(frames, start, Duration::from_micros(100), SR);
    }

    #[test]
    fn interval_load_follows_recent_callbacks_not_the_whole_run() {
        let stats = AudioStats::default();
        // 1000 frames (20,83 ms) usando 10 ms: carga ≈ 48 %.
        rec(&stats, 1000, Duration::from_millis(10));
        let first = stats.take_report(SR);
        assert!((first.interval_avg_load - 0.48).abs() < 0.01);

        // Un intervalo sin trabajo útil baja la carga, aunque la media total siga alta.
        rec(&stats, 1000, Duration::from_micros(100));
        let second = stats.take_report(SR);
        assert!(second.interval_avg_load < 0.01);
        assert!(second.avg_load > second.interval_avg_load);
    }

    #[test]
    fn counts_deadline_misses() {
        let stats = AudioStats::default();
        rec(&stats, 480, Duration::from_millis(20)); // buffer de 10 ms
        rec(&stats, 480, Duration::from_millis(1));
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

    #[test]
    fn steady_callbacks_have_no_late_ones() {
        let stats = AudioStats::default();
        // 128 frames a 48 kHz = 2,67 ms: callbacks cada ~2,67 ms (aquí cada 3 ms).
        for i in 0..50 {
            rec_at(&stats, 10 + i * 3, 128);
        }
        let r = stats.take_report(SR);
        assert_eq!(r.late, 0);
        assert!((r.max_gap_ms - 3.0).abs() < 0.01, "{}", r.max_gap_ms);
    }

    #[test]
    fn a_delayed_callback_counts_as_late_and_sets_the_max_gap() {
        let stats = AudioStats::default();
        rec_at(&stats, 10, 128);
        rec_at(&stats, 13, 128);
        rec_at(&stats, 30, 128); // 17 ms de hueco con un periodo de 2,67 ms
        rec_at(&stats, 33, 128);
        let r = stats.take_report(SR);
        assert_eq!(r.late, 1);
        assert!((r.max_gap_ms - 17.0).abs() < 0.01);
        assert!((r.interval_max_gap_ms - 17.0).abs() < 0.01);
        // El máximo del intervalo se reinicia; el total y el contador no.
        let r2 = stats.take_report(SR);
        assert_eq!(r2.interval_max_gap_ms, 0.0);
        assert!((r2.max_gap_ms - 17.0).abs() < 0.01);
        assert_eq!(r2.late, 1);
    }

    #[test]
    fn reopening_a_stream_does_not_count_the_closed_time_as_a_delay() {
        let stats = AudioStats::default();
        rec_at(&stats, 10, 128);
        rec_at(&stats, 13, 128);
        stats.reset_gap();
        rec_at(&stats, 5_000, 128); // el dispositivo estuvo cerrado casi 5 s
        rec_at(&stats, 5_003, 128);
        let r = stats.take_report(SR);
        assert_eq!(r.late, 0);
        assert!(r.max_gap_ms < 4.0);
    }

    #[test]
    fn realtime_status_encodes_every_outcome() {
        let stats = AudioStats::default();
        assert_eq!(stats.realtime(), Realtime::Pending);
        stats.record_realtime(70, &Ok(()));
        assert_eq!(stats.realtime(), Realtime::Active(70));
        stats.record_realtime(70, &Err(std::io::Error::from_raw_os_error(1)));
        assert_eq!(stats.realtime(), Realtime::Denied(1));
        stats.record_realtime(0, &Ok(()));
        assert_eq!(stats.realtime(), Realtime::Off);
        assert_eq!(stats.take_report(SR).realtime, Realtime::Off);
    }
}
