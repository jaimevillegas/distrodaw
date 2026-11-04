//! Banco de pruebas (Hito 1): N sinusoides simultáneas y métricas del callback.

use crate::audio::{self, AudioArgs};
use crate::stats::AudioStats;
use anyhow::Result;
use clap::Args;
use distrodaw_engine::{Event, ParamId, SineBank, Synth, VOICES};
use std::sync::Arc;
use std::time::Duration;

#[derive(Args, Debug)]
pub struct BenchArgs {
    /// Número de sinusoides simultáneas (prueba de polifonía).
    #[arg(long, default_value_t = 1)]
    voices: usize,

    /// Usa el sinte real con 8 notas sostenidas en lugar de sinusoides
    /// (`--voices` se ignora).
    #[arg(long)]
    synth: bool,

    /// Forma de onda del sinte (0 seno, 1 sierra, 2 cuadrada, 3 triangular).
    #[arg(long, default_value_t = 1.0)]
    waveform: f32,

    /// Duración de la prueba en segundos.
    #[arg(long, default_value_t = 3)]
    seconds: u64,
}

pub fn run(audio_args: &AudioArgs, args: &BenchArgs) -> Result<()> {
    let stats = Arc::new(AudioStats::default());
    let voices = args.voices;
    let (use_synth, waveform) = (args.synth, args.waveform);
    let output = audio::open(audio_args, &stats, true, move |rate| {
        let mut bank = SineBank::new(rate, if use_synth { 0 } else { voices }, 440.0);
        let mut synth = Synth::new(rate);
        // Acorde de 8 notas sostenidas, enviado en el primer callback.
        let mut chord = Some(chord_events(waveform));
        move |out: &mut [f32]| {
            if use_synth {
                let events = chord.take();
                synth.render(events.as_ref().map_or(&[][..], |e| &e[..]), out);
            } else {
                for sample in out.iter_mut() {
                    *sample = bank.next_sample() * 0.2;
                }
            }
        }
    })?;

    if use_synth {
        println!("Sonando el sinte con {VOICES} notas durante {} s...", args.seconds);
    } else {
        println!("Sonando {} voz/voces durante {} s...", args.voices, args.seconds);
    }
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

fn chord_events(waveform: f32) -> [Event; VOICES + 2] {
    const NOTES: [u8; VOICES] = [48, 52, 55, 59, 60, 64, 67, 71];
    let mut events = [Event::set_param(0, ParamId::Waveform, waveform); VOICES + 2];
    events[1] = Event::set_param(0, ParamId::Sustain, 1.0);
    for (slot, note) in events[2..].iter_mut().zip(NOTES) {
        *slot = Event::note_on(0, note, 100);
    }
    events
}
