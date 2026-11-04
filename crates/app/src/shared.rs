//! Estado que el hilo de audio publica para la interfaz. Todo son atómicos:
//! el audio solo escribe y la interfaz solo lee, sin locks ni reservas.

use distrodaw_engine::{ParamId, Params, PARAM_COUNT};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering::Relaxed};

pub struct Shared {
    params: [AtomicU32; PARAM_COUNT],
    /// Pico (valor absoluto) desde la última lectura, como bits de un `f32`.
    /// Para floats no negativos el orden de los bits coincide con el del valor,
    /// por lo que `fetch_max` sobre los bits calcula el máximo.
    peak: AtomicU32,
    voices: AtomicUsize,
}

impl Default for Shared {
    fn default() -> Self {
        let defaults = Params::default();
        Self {
            params: ParamId::ALL.map(|id| AtomicU32::new(defaults.get(id).to_bits())),
            peak: AtomicU32::new(0),
            voices: AtomicUsize::new(0),
        }
    }
}

impl Shared {
    // --- Lado del hilo de audio ---

    pub fn publish_params(&self, params: &Params) {
        for id in ParamId::ALL {
            self.params[id as usize].store(params.get(id).to_bits(), Relaxed);
        }
    }

    /// Registra el pico del bloque recién renderizado.
    pub fn publish_peak(&self, block: &[f32]) {
        let peak = block.iter().fold(0.0f32, |m, s| if s.is_finite() { m.max(s.abs()) } else { m });
        self.peak.fetch_max(peak.to_bits(), Relaxed);
    }

    pub fn publish_voices(&self, voices: usize) {
        self.voices.store(voices, Relaxed);
    }

    // --- Lado de la interfaz ---

    pub fn params(&self) -> [f32; PARAM_COUNT] {
        std::array::from_fn(|i| f32::from_bits(self.params[i].load(Relaxed)))
    }

    /// Pico desde la lectura anterior; reinicia el medidor.
    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Relaxed))
    }

    pub fn voices(&self) -> usize {
        self.voices.load(Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_default_parameters() {
        let shared = Shared::default();
        for id in ParamId::ALL {
            assert_eq!(shared.params()[id as usize], id.spec().default);
        }
    }

    #[test]
    fn published_parameters_are_visible() {
        let shared = Shared::default();
        let mut params = Params::default();
        params.set(ParamId::Volume, 0.9);
        shared.publish_params(&params);
        assert_eq!(shared.params()[ParamId::Volume as usize], 0.9);
    }

    #[test]
    fn peak_keeps_the_maximum_until_taken() {
        let shared = Shared::default();
        shared.publish_peak(&[0.1, -0.5, 0.2]);
        shared.publish_peak(&[0.3]);
        assert_eq!(shared.take_peak(), 0.5);
        assert_eq!(shared.take_peak(), 0.0);
        shared.publish_peak(&[0.25]);
        assert_eq!(shared.take_peak(), 0.25);
    }

    #[test]
    fn non_finite_samples_do_not_poison_the_peak() {
        let shared = Shared::default();
        shared.publish_peak(&[f32::NAN, f32::INFINITY, 0.4]);
        assert_eq!(shared.take_peak(), 0.4);
    }

    #[test]
    fn voices_are_shared() {
        let shared = Shared::default();
        shared.publish_voices(5);
        assert_eq!(shared.voices(), 5);
    }
}
