//! Una voz: oscilador con forma de onda seleccionable y envolvente ADSR.

use crate::env::{Adsr, EnvCoefs};
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

impl Waveform {
    /// Convierte el valor del parámetro `waveform` (0 a 3) en una forma de onda.
    pub fn from_param(value: f32) -> Self {
        match value.round() as i32 {
            0 => Waveform::Sine,
            1 => Waveform::Saw,
            2 => Waveform::Square,
            _ => Waveform::Triangle,
        }
    }

    /// Muestra para una fase en [0.0, 1.0). Formas ingenuas (sin anti-aliasing).
    #[inline]
    pub fn sample(self, phase: f32) -> f32 {
        match self {
            Waveform::Sine => (phase * TAU).sin(),
            Waveform::Saw => 2.0 * phase - 1.0,
            Waveform::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        }
    }
}

/// Frecuencia en Hz de una nota MIDI (69 = La4 = 440 Hz).
pub fn midi_to_hz(note: u8) -> f32 {
    440.0 * 2f32.powf((f32::from(note) - 69.0) / 12.0)
}

#[derive(Debug, Clone, Copy)]
pub struct Voice {
    phase: f32,
    phase_inc: f32,
    note: u8,
    gain: f32,
    gate: bool,
    age: u64,
    env: Adsr,
}

impl Default for Voice {
    fn default() -> Self {
        Self { phase: 0.0, phase_inc: 0.0, note: 0, gain: 0.0, gate: false, age: 0, env: Adsr::default() }
    }
}

impl Voice {
    /// Se mantiene la fase al reutilizar la voz para evitar saltos en la señal.
    pub fn note_on(&mut self, sample_rate: f32, note: u8, velocity: u8, age: u64) {
        self.note = note;
        self.phase_inc = midi_to_hz(note) / sample_rate;
        self.gain = f32::from(velocity.min(127)) / 127.0;
        self.gate = true;
        self.age = age;
        self.env.trigger();
    }

    pub fn note_off(&mut self) {
        self.gate = false;
        self.env.release();
    }

    pub fn note(&self) -> u8 {
        self.note
    }

    /// `true` mientras la nota está pulsada.
    pub fn gate(&self) -> bool {
        self.gate
    }

    pub fn age(&self) -> u64 {
        self.age
    }

    /// `true` mientras suena, incluida la relajación.
    pub fn is_active(&self) -> bool {
        !self.env.is_idle()
    }

    /// Suma la salida de la voz en `out`.
    #[inline]
    pub fn render(&mut self, waveform: Waveform, coefs: &EnvCoefs, out: &mut [f32]) {
        for sample in out.iter_mut() {
            let level = self.env.next(coefs);
            *sample += waveform.sample(self.phase) * level * self.gain;
            self.phase += self.phase_inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn midi_pitch_reference_points() {
        assert!((midi_to_hz(69) - 440.0).abs() < 1e-3);
        assert!((midi_to_hz(81) - 880.0).abs() < 1e-2);
        assert!((midi_to_hz(60) - 261.626).abs() < 1e-2);
    }

    #[test]
    fn waveforms_stay_in_range_and_have_expected_shape() {
        for wf in [Waveform::Sine, Waveform::Saw, Waveform::Square, Waveform::Triangle] {
            for i in 0..1000 {
                let v = wf.sample(i as f32 / 1000.0);
                assert!((-1.0..=1.0).contains(&v), "{wf:?} fuera de rango: {v}");
            }
        }
        assert_eq!(Waveform::Square.sample(0.25), 1.0);
        assert_eq!(Waveform::Square.sample(0.75), -1.0);
        assert_eq!(Waveform::Triangle.sample(0.5), 1.0);
        assert_eq!(Waveform::Triangle.sample(0.0), -1.0);
    }

    #[test]
    fn param_values_select_waveforms() {
        assert_eq!(Waveform::from_param(0.0), Waveform::Sine);
        assert_eq!(Waveform::from_param(1.2), Waveform::Saw);
        assert_eq!(Waveform::from_param(2.0), Waveform::Square);
        assert_eq!(Waveform::from_param(3.0), Waveform::Triangle);
    }

    #[test]
    fn voice_sounds_then_goes_idle_after_release() {
        let coefs = EnvCoefs::new(SR, 0.001, 0.01, 0.8, 0.02);
        let mut voice = Voice::default();
        assert!(!voice.is_active());

        voice.note_on(SR, 69, 127, 1);
        let mut buf = [0.0; 480];
        voice.render(Waveform::Sine, &coefs, &mut buf);
        assert!(buf.iter().any(|s| s.abs() > 0.1));
        assert!(voice.is_active() && voice.gate());

        voice.note_off();
        assert!(!voice.gate());
        for _ in 0..50 {
            voice.render(Waveform::Sine, &coefs, &mut buf);
        }
        assert!(!voice.is_active());
    }

    #[test]
    fn velocity_scales_amplitude() {
        let coefs = EnvCoefs::new(SR, 0.001, 0.01, 1.0, 0.1);
        let peak = |vel| {
            let mut v = Voice::default();
            v.note_on(SR, 69, vel, 1);
            let mut buf = [0.0; 960];
            v.render(Waveform::Sine, &coefs, &mut buf);
            buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
        };
        assert!(peak(127) > peak(64) * 1.8);
    }
}
