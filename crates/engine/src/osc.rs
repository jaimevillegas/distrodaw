use std::f32::consts::TAU;

/// Oscilador sinusoidal simple (Hito 0).
#[derive(Debug, Clone)]
pub struct Sine {
    phase: f32,
    phase_inc: f32,
    sample_rate: f32,
}

impl Sine {
    pub fn new(sample_rate: f32, freq_hz: f32) -> Self {
        let mut s = Self { phase: 0.0, phase_inc: 0.0, sample_rate };
        s.set_freq(freq_hz);
        s
    }

    pub fn set_freq(&mut self, freq_hz: f32) {
        self.phase_inc = freq_hz / self.sample_rate;
    }

    /// Siguiente muestra en el rango [-1.0, 1.0].
    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        let out = (self.phase * TAU).sin();
        self.phase += self.phase_inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_in_range() {
        let mut s = Sine::new(48_000.0, 440.0);
        for _ in 0..48_000 {
            let v = s.next_sample();
            assert!((-1.0..=1.0).contains(&v));
        }
    }

    #[test]
    fn completes_a_cycle_per_period() {
        // 1 Hz a 100 Hz de muestreo: tras 100 muestras vuelve a la fase inicial.
        let mut s = Sine::new(100.0, 1.0);
        let first = s.next_sample();
        for _ in 0..99 {
            s.next_sample();
        }
        let again = s.next_sample();
        assert!((first - again).abs() < 1e-3);
    }
}
