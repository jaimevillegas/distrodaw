//! Hito 1: banco de pruebas de audio. Toca una o varias sinusoides por la
//! salida elegida y mide cuánto tarda el callback de audio.

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, SampleFormat, SampleRate, SizedSample};
use distrodaw_engine::Sine;
use std::time::Duration;

/// Banco de pruebas de audio de DistroDAW (Hito 1).
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Lista los dispositivos de salida y termina.
    #[arg(long)]
    list_devices: bool,

    /// Fragmento del nombre del dispositivo de salida (por defecto, el predeterminado).
    #[arg(long)]
    device: Option<String>,

    /// Tamaño del buffer en frames (por defecto, el del dispositivo).
    #[arg(long)]
    buffer: Option<u32>,

    /// Frecuencia de muestreo en Hz (por defecto, la del dispositivo).
    #[arg(long)]
    rate: Option<u32>,

    /// Duración de la prueba en segundos.
    #[arg(long, default_value_t = 3)]
    seconds: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let host = cpal::default_host();

    if args.list_devices {
        list_devices(&host)?;
        return Ok(());
    }

    let device = pick_device(&host, args.device.as_deref())?;
    println!("Dispositivo: {}", device.name().unwrap_or_else(|_| "?".into()));

    let supported = device.default_output_config().context("config de salida")?;
    let sample_format = supported.sample_format();
    let mut config = supported.config();
    if let Some(rate) = args.rate {
        config.sample_rate = SampleRate(rate);
    }
    if let Some(frames) = args.buffer {
        config.buffer_size = BufferSize::Fixed(frames);
    }
    println!("Config: {config:?} ({sample_format:?})");

    let stream = match sample_format {
        SampleFormat::F32 => build::<f32>(&device, &config)?,
        SampleFormat::I16 => build::<i16>(&device, &config)?,
        SampleFormat::U16 => build::<u16>(&device, &config)?,
        other => return Err(anyhow!("formato de muestra no soportado: {other:?}")),
    };

    stream.play()?;
    println!("Sonando 440 Hz durante {} s...", args.seconds);
    std::thread::sleep(Duration::from_secs(args.seconds));
    Ok(())
}

fn list_devices(host: &cpal::Host) -> Result<()> {
    let default = host.default_output_device().and_then(|d| d.name().ok());
    for device in host.output_devices().context("listando dispositivos")? {
        let name = device.name().unwrap_or_else(|_| "?".into());
        let mark = if default.as_deref() == Some(name.as_str()) { " (por defecto)" } else { "" };
        println!("{name}{mark}");
    }
    Ok(())
}

fn pick_device(host: &cpal::Host, filter: Option<&str>) -> Result<cpal::Device> {
    let Some(filter) = filter else {
        return host
            .default_output_device()
            .ok_or_else(|| anyhow!("no hay dispositivo de salida de audio"));
    };
    let filter = filter.to_lowercase();
    host.output_devices()
        .context("listando dispositivos")?
        .find(|d| d.name().is_ok_and(|n| n.to_lowercase().contains(&filter)))
        .ok_or_else(|| anyhow!("ningún dispositivo coincide con {filter:?}; usa --list-devices"))
}

fn build<T>(device: &cpal::Device, config: &cpal::StreamConfig) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;
    let mut osc = Sine::new(sample_rate, 440.0);

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            // Hilo de audio: sin reservas de memoria ni locks.
            for frame in data.chunks_mut(channels) {
                let v = T::from_sample(osc.next_sample() * 0.2);
                for out in frame.iter_mut() {
                    *out = v;
                }
            }
        },
        |err| eprintln!("error de audio: {err}"),
        None,
    )?;
    Ok(stream)
}
