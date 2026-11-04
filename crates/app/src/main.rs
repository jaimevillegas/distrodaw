//! Hito 0: una sinusoide de 440 Hz por la salida de audio por defecto.

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use distrodaw_engine::Sine;
use std::time::Duration;

fn main() -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow!("no hay dispositivo de salida de audio"))?;
    println!("Dispositivo: {}", device.name().unwrap_or_else(|_| "?".into()));

    let supported = device.default_output_config().context("config de salida")?;
    println!("Config: {:?}", supported);

    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, &config)?,
        SampleFormat::I16 => build::<i16>(&device, &config)?,
        SampleFormat::U16 => build::<u16>(&device, &config)?,
        other => return Err(anyhow!("formato de muestra no soportado: {other:?}")),
    };

    stream.play()?;
    println!("Sonando 440 Hz durante 3 segundos...");
    std::thread::sleep(Duration::from_secs(3));
    Ok(())
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
