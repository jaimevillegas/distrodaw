use crate::osc::Sine;

/// Banco de sinusoides para medir cuántas voces caben en una CPU (Hito 1).
///
/// Reserva toda su memoria al construirse; `next_sample` no reserva nada y es
/// seguro llamarlo desde el hilo de audio.
#[derive(Debug, Clone)]
pub struct SineBank {
    voices: Vec<Sine>,
    gain: f32,
}

impl SineBank {
    /// `count` sinusoides repartidas en dos octavas a partir de `base_hz`
    /// (la primera suena exactamente a `base_hz`).
    pub fn new(sample_rate: f32, count: usize, base_hz: f32) -> Self {
        let voices = (0..count)
            .map(|i| Sine::new(sample_rate, base_hz * 2f32.powf((i % 24) as f32 / 12.0)))
            .collect();
        let gain = if count == 0 { 0.0 } else { 1.0 / count as f32 };
        Self { voices, gain }
    }

    pub fn len(&self) -> usize {
        self.voices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.voices.is_empty()
    }

    /// Mezcla de todas las voces, normalizada al rango [-1.0, 1.0].
    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        let mut sum = 0.0;
        for v in &mut self.voices {
            sum += v.next_sample();
        }
        sum * self.gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_stays_in_range() {
        let mut bank = SineBank::new(48_000.0, 64, 110.0);
        for _ in 0..48_000 {
            assert!((-1.0..=1.0).contains(&bank.next_sample()));
        }
    }

    #[test]
    fn single_voice_matches_sine() {
        let mut bank = SineBank::new(48_000.0, 1, 440.0);
        let mut sine = Sine::new(48_000.0, 440.0);
        for _ in 0..1_000 {
            assert_eq!(bank.next_sample(), sine.next_sample());
        }
    }

    #[test]
    fn empty_bank_is_silent() {
        let mut bank = SineBank::new(48_000.0, 0, 440.0);
        assert!(bank.is_empty());
        assert_eq!(bank.next_sample(), 0.0);
    }
}
