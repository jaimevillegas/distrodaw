//! Parámetros con identidad: cada control del sinte tiene ID, nombre, rango y
//! valor. TUI, joystick, MIDI CC y patches controlan los mismos parámetros.

/// Identificador estable de un parámetro. El número es el índice en [`SPECS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ParamId {
    /// Forma de onda: 0 seno, 1 sierra, 2 cuadrada, 3 triangular.
    Waveform = 0,
    /// Tiempo de ataque en segundos.
    Attack = 1,
    /// Tiempo de caída hasta el nivel de sustain, en segundos.
    Decay = 2,
    /// Nivel de sustain (0.0 a 1.0).
    Sustain = 3,
    /// Tiempo de relajación tras soltar la nota, en segundos.
    Release = 4,
    /// Volumen general (0.0 a 1.0).
    Volume = 5,
}

pub const PARAM_COUNT: usize = 6;

/// Cómo se reparte el rango de un parámetro al moverlo con un control normalizado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// Reparto lineal.
    Linear,
    /// Tiempos: curva cuadrática, con más resolución en valores cortos.
    Time,
    /// Solo valores enteros del rango (p. ej. la forma de onda).
    Stepped,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    pub id: ParamId,
    pub kind: ParamKind,
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

/// Tabla de parámetros, ordenada por ID.
pub const SPECS: [ParamSpec; PARAM_COUNT] = [
    ParamSpec { id: ParamId::Waveform, kind: ParamKind::Stepped, name: "waveform", min: 0.0, max: 3.0, default: 1.0 },
    ParamSpec { id: ParamId::Attack, kind: ParamKind::Time, name: "attack", min: 0.001, max: 5.0, default: 0.01 },
    ParamSpec { id: ParamId::Decay, kind: ParamKind::Time, name: "decay", min: 0.001, max: 5.0, default: 0.2 },
    ParamSpec { id: ParamId::Sustain, kind: ParamKind::Linear, name: "sustain", min: 0.0, max: 1.0, default: 0.7 },
    ParamSpec { id: ParamId::Release, kind: ParamKind::Time, name: "release", min: 0.001, max: 5.0, default: 0.3 },
    ParamSpec { id: ParamId::Volume, kind: ParamKind::Linear, name: "volume", min: 0.0, max: 1.0, default: 0.5 },
];

impl ParamSpec {
    /// Posición del valor en el rango como un número de 0.0 a 1.0.
    pub fn to_normalized(&self, value: f32) -> f32 {
        let linear = ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0);
        match self.kind {
            ParamKind::Time => linear.sqrt(),
            ParamKind::Linear | ParamKind::Stepped => linear,
        }
    }

    /// Valor del parámetro para una posición normalizada (0.0 a 1.0), según su [`ParamKind`].
    pub fn from_normalized(&self, norm: f32) -> f32 {
        let norm = norm.clamp(0.0, 1.0);
        let span = self.max - self.min;
        match self.kind {
            ParamKind::Linear => self.min + norm * span,
            ParamKind::Time => self.min + norm * norm * span,
            ParamKind::Stepped => (self.min + norm * span).round(),
        }
    }
}

impl ParamId {
    pub const ALL: [ParamId; PARAM_COUNT] = [
        ParamId::Waveform,
        ParamId::Attack,
        ParamId::Decay,
        ParamId::Sustain,
        ParamId::Release,
        ParamId::Volume,
    ];

    pub fn spec(self) -> &'static ParamSpec {
        &SPECS[self as usize]
    }

    pub fn from_index(index: usize) -> Option<ParamId> {
        Self::ALL.get(index).copied()
    }
}

/// Valores actuales de todos los parámetros. Tamaño fijo: no reserva memoria.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    values: [f32; PARAM_COUNT],
}

impl Default for Params {
    fn default() -> Self {
        Self { values: SPECS.map(|s| s.default) }
    }
}

impl Params {
    pub fn get(&self, id: ParamId) -> f32 {
        self.values[id as usize]
    }

    /// Fija el valor limitándolo al rango del parámetro.
    pub fn set(&mut self, id: ParamId, value: f32) {
        let spec = id.spec();
        let value = if value.is_nan() { spec.default } else { value };
        self.values[id as usize] = value.clamp(spec.min, spec.max);
    }

    /// Valor normalizado (0.0 a 1.0) respecto al rango del parámetro, útil
    /// para controladores como CC MIDI, joystick o barras de la TUI.
    pub fn set_normalized(&mut self, id: ParamId, norm: f32) {
        self.set(id, id.spec().from_normalized(norm));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_are_ordered_by_id() {
        for (i, spec) in SPECS.iter().enumerate() {
            assert_eq!(spec.id as usize, i);
            assert_eq!(ParamId::from_index(i), Some(spec.id));
            assert!(spec.min <= spec.default && spec.default <= spec.max);
        }
        assert_eq!(ParamId::from_index(PARAM_COUNT), None);
    }

    #[test]
    fn defaults_match_specs() {
        let p = Params::default();
        for id in ParamId::ALL {
            assert_eq!(p.get(id), id.spec().default);
        }
    }

    #[test]
    fn set_clamps_to_range() {
        let mut p = Params::default();
        p.set(ParamId::Sustain, 7.0);
        assert_eq!(p.get(ParamId::Sustain), 1.0);
        p.set(ParamId::Attack, -1.0);
        assert_eq!(p.get(ParamId::Attack), ParamId::Attack.spec().min);
        p.set(ParamId::Volume, f32::NAN);
        assert_eq!(p.get(ParamId::Volume), ParamId::Volume.spec().default);
    }

    #[test]
    fn normalization_round_trips_for_every_kind() {
        for spec in SPECS {
            for norm in [0.0, 0.25, 0.5, 1.0] {
                let value = spec.from_normalized(norm);
                assert!(value >= spec.min && value <= spec.max);
                let back = spec.to_normalized(value);
                if spec.kind == ParamKind::Stepped {
                    assert!((spec.from_normalized(back) - value).abs() < 1e-6);
                } else {
                    assert!((back - norm).abs() < 1e-4, "{}: {norm} -> {back}", spec.name);
                }
            }
        }
    }

    #[test]
    fn time_parameters_are_finer_at_short_values() {
        let spec = ParamId::Attack.spec();
        let linear_half = spec.min + 0.5 * (spec.max - spec.min);
        assert!(spec.from_normalized(0.5) < linear_half * 0.6);
    }

    #[test]
    fn stepped_parameters_only_take_whole_values() {
        let spec = ParamId::Waveform.spec();
        for i in 0..=100 {
            let v = spec.from_normalized(i as f32 / 100.0);
            assert_eq!(v, v.round());
        }
    }

    #[test]
    fn normalized_maps_to_range() {
        let mut p = Params::default();
        p.set_normalized(ParamId::Release, 0.0);
        assert_eq!(p.get(ParamId::Release), ParamId::Release.spec().min);
        p.set_normalized(ParamId::Release, 1.0);
        assert_eq!(p.get(ParamId::Release), ParamId::Release.spec().max);
        p.set_normalized(ParamId::Volume, 0.5);
        assert!((p.get(ParamId::Volume) - 0.5).abs() < 1e-6);
    }
}
