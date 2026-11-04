//! DistroDAW: DAW solo MIDI para terminal.

mod audio;
mod bench;
mod midi;
mod patches;
mod play;
mod session;
mod shared;
mod stats;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    #[command(flatten)]
    audio: audio::AudioArgs,

    /// Sin subcomando se abre la interfaz.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Lista los dispositivos de salida de audio.
    Devices,
    /// Lista los puertos de entrada MIDI.
    MidiPorts,
    /// Banco de pruebas: sinusoides simultáneas y métricas de CPU.
    Bench(bench::BenchArgs),
    /// Toca el sinte con un teclado MIDI, en modo de texto.
    Play(play::PlayArgs),
    /// Interfaz de terminal (es lo que se abre sin subcomando).
    Tui(tui::TuiArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or_else(|| Command::Tui(tui::TuiArgs::default())) {
        Command::Devices => audio::list_devices(),
        Command::MidiPorts => midi::list_ports(),
        Command::Bench(args) => bench::run(&cli.audio, &args),
        Command::Play(args) => play::run(&cli.audio, &args),
        Command::Tui(args) => tui::run(&cli.audio, &args),
    }
}
