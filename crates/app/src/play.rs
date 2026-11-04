//! Modo de tocar: teclado MIDI → cola sin bloqueo → sinte de 8 voces → audio.

use crate::audio::{self, AudioArgs};
use crate::midi;
use crate::stats::AudioStats;
use anyhow::{anyhow, Result};
use clap::Args;
use distrodaw_engine::{Event, Synth};
use midir::MidiInput;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Duration;

/// Eventos que el hilo de audio atiende por trozo. El resto espera al siguiente.
const BATCH: usize = 64;
const QUEUE: usize = 256;

#[derive(Args, Debug)]
pub struct PlayArgs {
    /// Fragmento del nombre del puerto MIDI (por defecto, el primer teclado).
    #[arg(long)]
    midi: Option<String>,

    /// Imprime cada mensaje MIDI recibido.
    #[arg(long)]
    monitor: bool,

    /// Segundos que dura la sesión (0 = hasta pulsar Ctrl+C).
    #[arg(long, default_value_t = 0)]
    seconds: u64,
}

pub fn run(audio_args: &AudioArgs, args: &PlayArgs) -> Result<()> {
    let (mut producer, mut consumer) = rtrb::RingBuffer::<Event>::new(QUEUE);
    let stats = Arc::new(AudioStats::default());
    let active = Arc::new(AtomicUsize::new(0));

    let audio_active = Arc::clone(&active);
    let output = audio::open(audio_args, &stats, move |rate| {
        let mut synth = Synth::new(rate);
        let mut events = [Event::note_off(0, 0); BATCH];
        move |out: &mut [f32]| {
            let mut n = 0;
            while n < BATCH {
                match consumer.pop() {
                    Ok(event) => {
                        events[n] = event;
                        n += 1;
                    }
                    Err(_) => break,
                }
            }
            synth.render(&events[..n], out);
            audio_active.store(synth.active_voices(), Relaxed);
        }
    })?;

    let input = MidiInput::new("distrodaw")?;
    let port = midi::pick_port(&input, args.midi.as_deref())?;
    let port_name = input.port_name(&port).map_err(|e| anyhow!("{e}"))?;
    let monitor = args.monitor;
    let _connection = input
        .connect(
            &port,
            "distrodaw-in",
            move |_timestamp, bytes, _| {
                let Some(msg) = midi::parse(bytes) else { return };
                if monitor {
                    println!("{msg:?}");
                }
                // Los eventos en vivo caen en el frame 0 del siguiente trozo de
                // audio (< 1 ms con buffers de 64 frames). Si la cola se llena, se descarta.
                if let Some(kind) = midi::to_event(msg) {
                    let _ = producer.push(Event::new(0, kind));
                }
            },
            (),
        )
        .map_err(|e| anyhow!("no se pudo abrir el puerto MIDI: {e}"))?;
    println!("MIDI: {port_name}");
    println!("Toca el teclado. Ctrl+C para salir.");

    let rate = output.sample_rate;
    let started = std::time::Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let r = stats.take_report(rate);
        print!(
            "\rvoces {}/8 | carga media {:.1} % | pico {:.2} ms | plazos perdidos {} | errores {}   ",
            active.load(Relaxed),
            r.avg_load * 100.0,
            r.peak_ms,
            r.misses,
            r.stream_errors,
        );
        std::io::stdout().flush()?;
        if args.seconds > 0 && started.elapsed().as_secs() >= args.seconds {
            println!();
            return Ok(());
        }
    }
}
