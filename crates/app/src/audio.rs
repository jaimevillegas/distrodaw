//! Apertura de la salida de audio, común al banco de pruebas y al sinte.

use crate::stats::AudioStats;
use anyhow::{anyhow, Context, Result};
use clap::Args;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, SampleFormat, SampleRate, SizedSample};
use std::sync::Arc;
use std::time::Instant;

/// El audio se procesa en trozos mono de este tamaño máximo; el búfer de
/// trabajo se reserva una vez, antes de arrancar el stream.
const CHUNK: usize = 4096;

#[derive(Args, Debug, Clone)]
pub struct AudioArgs {
    /// Fragmento del nombre del dispositivo de salida (por defecto, el predeterminado).
    #[arg(long, global = true)]
    pub device: Option<String>,

    /// Tamaño del buffer en frames (por defecto, el del dispositivo).
    #[arg(long, global = true)]
    pub buffer: Option<u32>,

    /// Frecuencia de muestreo en Hz (por defecto, la del dispositivo).
    #[arg(long, global = true)]
    pub rate: Option<u32>,
}

/// Stream de salida abierto. Mantenerlo vivo mantiene el audio sonando.
pub struct Output {
    _stream: cpal::Stream,
    pub device_name: String,
    pub sample_rate: u32,
    /// Buffer pedido con `--buffer`; `None` si se usa el del dispositivo.
    pub device_buffer: Option<u64>,
}

pub fn list_devices() -> Result<()> {
    let host = cpal::default_host();
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
        .ok_or_else(|| anyhow!("ningún dispositivo coincide con {filter:?}; usa `devices`"))
}

/// Abre la salida y empieza a sonar. `make_render` recibe la frecuencia de
/// muestreo y devuelve la función que rellena cada trozo mono; esa función se
/// ejecuta en el hilo de audio y no debe reservar memoria ni usar locks.
/// Con `verbose` a `false` no escribe nada en la consola (lo necesita la TUI).
pub fn open<R>(
    args: &AudioArgs,
    stats: &Arc<AudioStats>,
    verbose: bool,
    make_render: impl FnOnce(f32) -> R,
) -> Result<Output>
where
    R: FnMut(&mut [f32]) + Send + 'static,
{
    let host = cpal::default_host();
    let device = pick_device(&host, args.device.as_deref())?;
    let device_name = device.name().unwrap_or_else(|_| "?".into());
    if verbose {
        println!("Dispositivo: {device_name}");
    }

    let supported = device.default_output_config().context("config de salida")?;
    let sample_format = supported.sample_format();
    let mut config = supported.config();
    if let Some(rate) = args.rate {
        config.sample_rate = SampleRate(rate);
    }
    if let Some(frames) = args.buffer {
        config.buffer_size = BufferSize::Fixed(frames);
    }
    if verbose {
        println!("Config: {config:?} ({sample_format:?})");
    }

    let render = make_render(config.sample_rate.0 as f32);
    let stream = match sample_format {
        SampleFormat::F32 => build::<f32, R>(&device, &config, stats, verbose, render)?,
        SampleFormat::I16 => build::<i16, R>(&device, &config, stats, verbose, render)?,
        SampleFormat::U16 => build::<u16, R>(&device, &config, stats, verbose, render)?,
        other => return Err(anyhow!("formato de muestra no soportado: {other:?}")),
    };
    stream.play()?;

    let device_buffer = match config.buffer_size {
        BufferSize::Fixed(frames) => Some(u64::from(frames)),
        BufferSize::Default => None,
    };
    Ok(Output { _stream: stream, device_name, sample_rate: config.sample_rate.0, device_buffer })
}

fn build<T, R>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    stats: &Arc<AudioStats>,
    verbose: bool,
    mut render: R,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
    R: FnMut(&mut [f32]) + Send + 'static,
{
    let rate = config.sample_rate.0;
    let channels = config.channels as usize;
    let mut mono = vec![0.0f32; CHUNK];
    let cb_stats = Arc::clone(stats);
    let err_stats = Arc::clone(stats);

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            // Hilo de audio: sin reservas de memoria ni locks.
            let start = Instant::now();
            let frames = data.len() / channels;
            let mut done = 0;
            while done < frames {
                let n = (frames - done).min(CHUNK);
                render(&mut mono[..n]);
                let out = &mut data[done * channels..(done + n) * channels];
                for (frame, &v) in out.chunks_mut(channels).zip(&mono[..n]) {
                    let sample = T::from_sample(v);
                    for slot in frame.iter_mut() {
                        *slot = sample;
                    }
                }
                done += n;
            }
            cb_stats.record(frames, start.elapsed(), rate);
        },
        move |err| {
            err_stats.record_stream_error();
            if verbose {
                eprintln!("error de audio: {err}");
            }
        },
        None,
    )?;
    Ok(stream)
}
