//! Modo de texto: tocar con el teclado MIDI y ver una línea de estado.

use crate::audio::AudioArgs;
use crate::patches;
use crate::session::{self, Mode};
use anyhow::Result;
use clap::Args;
use std::io::Write;
use std::time::Duration;

#[derive(Args, Debug)]
pub struct PlayArgs {
    /// Fragmento del nombre del puerto MIDI (por defecto, el primer teclado).
    #[arg(long)]
    midi: Option<String>,

    /// Imprime cada mensaje MIDI recibido.
    #[arg(long)]
    monitor: bool,

    /// Patch (.toml) que se carga al arrancar.
    #[arg(long)]
    patch: Option<std::path::PathBuf>,

    /// Segundos que dura la sesión (0 = hasta pulsar Ctrl+C).
    #[arg(long, default_value_t = 0)]
    seconds: u64,
}

pub fn run(audio_args: &AudioArgs, args: &PlayArgs) -> Result<()> {
    let mut session = session::start(audio_args, args.midi.as_deref(), true, Mode::Text { monitor: args.monitor })?;
    if let Some(path) = &args.patch {
        let patch = patches::load(path).map_err(anyhow::Error::msg)?;
        patches::send(&patch, &mut session.ui_tx);
        println!("Patch: {}", patch.name);
    }
    println!("MIDI: {}", session.midi_port.as_deref().unwrap_or("-"));
    println!("Toca el teclado. Ctrl+C para salir.");

    let rate = session.output.info.sample_rate;
    let started = std::time::Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(500));
        // Nadie lee los mensajes en este modo: se vacía la cola para que no se llene.
        while session.midi_rx.pop().is_ok() {}
        let r = session.stats.take_report(rate);
        print!(
            "\rvoces {}/8 | carga {:.1} % | pico {:.2} ms | plazos perdidos {} | errores {}   ",
            session.shared.voices(),
            r.interval_avg_load * 100.0,
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
