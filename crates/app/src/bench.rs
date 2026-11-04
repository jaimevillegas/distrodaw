//! Banco de pruebas (Hito 1): N sinusoides simultáneas y métricas del callback.

use crate::audio::{self, AudioArgs};
use crate::stats::AudioStats;
use anyhow::Result;
use clap::Args;
use distrodaw_engine::SineBank;
use std::sync::Arc;
use std::time::Duration;

#[derive(Args, Debug)]
pub struct BenchArgs {
    /// Número de sinusoides simultáneas (prueba de polifonía).
    #[arg(long, default_value_t = 1)]
    voices: usize,

    /// Duración de la prueba en segundos.
    #[arg(long, default_value_t = 3)]
    seconds: u64,
}

pub fn run(audio_args: &AudioArgs, args: &BenchArgs) -> Result<()> {
    let stats = Arc::new(AudioStats::default());
    let voices = args.voices;
    let output = audio::open(audio_args, &stats, |rate| {
        let mut bank = SineBank::new(rate, voices, 440.0);
        move |out: &mut [f32]| {
            for sample in out.iter_mut() {
                *sample = bank.next_sample() * 0.2;
            }
        }
    })?;

    println!("Sonando {} voz/voces durante {} s...", args.voices, args.seconds);
    let rate = output.sample_rate;
    let buffer_ms = |frames: u64| frames as f64 * 1000.0 / f64::from(rate);
    for _ in 0..args.seconds {
        std::thread::sleep(Duration::from_secs(1));
        let r = stats.take_report(rate);
        // Sin buffer fijo se desconoce el del dispositivo: se muestra el
        // tamaño del último callback como referencia.
        let buffer = output.device_buffer.unwrap_or(r.frames_per_callback);
        println!(
            "buffer {} frames ({:.2} ms) | callback {} frames | carga media {:.1} % | pico del intervalo {:.1} % | plazos perdidos {} | errores {}",
            buffer,
            buffer_ms(buffer),
            r.frames_per_callback,
            r.avg_load * 100.0,
            r.interval_max_load * 100.0,
            r.misses,
            r.stream_errors,
        );
    }

    let r = stats.take_report(rate);
    let buffer = output.device_buffer.unwrap_or(r.frames_per_callback);
    println!(
        "RESUMEN callbacks={} buffer_frames={} buffer_ms={:.2} callback_frames={} carga_media={:.1}% pico={:.1}% pico_ms={:.2} plazos_perdidos={} errores={}",
        r.callbacks,
        buffer,
        buffer_ms(buffer),
        r.frames_per_callback,
        r.avg_load * 100.0,
        r.peak_load * 100.0,
        r.peak_ms,
        r.misses,
        r.stream_errors,
    );
    Ok(())
}
