//! DistroDAW: DAW solo MIDI para terminal.

mod audio;
mod bench;
mod stats;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    #[command(flatten)]
    audio: audio::AudioArgs,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Lista los dispositivos de salida de audio.
    Devices,
    /// Banco de pruebas: sinusoides simultáneas y métricas de CPU.
    Bench(bench::BenchArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Devices => audio::list_devices(),
        Command::Bench(args) => bench::run(&cli.audio, &args),
    }
}
