//! Patches: un sinte descrito en un archivo de texto (TOML), editable sin
//! recompilar. Este módulo solo convierte texto ↔ [`Patch`]; leer y escribir
//! archivos es cosa de la app.
//!
//! ```toml
//! name = "Pad01"
//!
//! [oscillator]
//! waveform = "saw"      # sine, saw, square o triangle
//!
//! [envelope]
//! attack = 0.4          # segundos
//! decay = 0.5
//! sustain = 0.8         # 0.0 a 1.0
//! release = 1.2
//!
//! [output]
//! volume = 0.5          # 0.0 a 1.0
//! ```
//!
//! Todo es opcional: lo que falte toma el valor por defecto. Una clave
//! desconocida o un valor fuera de rango es un error, para que un fallo al
//! escribir a mano se vea en vez de ignorarse.

use crate::params::{ParamId, Params};
use serde::Deserialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    pub name: String,
    pub params: Params,
}

impl Default for Patch {
    fn default() -> Self {
        Self { name: "Init".into(), params: Params::default() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatchError {
    /// TOML mal formado, clave desconocida o tipo equivocado.
    Syntax(String),
    OutOfRange { field: &'static str, value: f32, min: f32, max: f32 },
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatchError::Syntax(msg) => write!(f, "{msg}"),
            PatchError::OutOfRange { field, value, min, max } => {
                write!(f, "{field} = {value} fuera de rango ({min} a {max})")
            }
        }
    }
}

impl std::error::Error for PatchError {}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum WaveformName {
    Sine,
    Saw,
    Square,
    Triangle,
}

impl WaveformName {
    fn value(&self) -> f32 {
        match self {
            WaveformName::Sine => 0.0,
            WaveformName::Saw => 1.0,
            WaveformName::Square => 2.0,
            WaveformName::Triangle => 3.0,
        }
    }
}

fn waveform_text(value: f32) -> &'static str {
    match value.round() as i32 {
        0 => "sine",
        1 => "saw",
        2 => "square",
        _ => "triangle",
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct PatchFile {
    name: Option<String>,
    oscillator: OscillatorSection,
    envelope: EnvelopeSection,
    output: OutputSection,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct OscillatorSection {
    waveform: Option<WaveformName>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct EnvelopeSection {
    attack: Option<f32>,
    decay: Option<f32>,
    sustain: Option<f32>,
    release: Option<f32>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct OutputSection {
    volume: Option<f32>,
}

impl Patch {
    pub fn from_toml(text: &str) -> Result<Patch, PatchError> {
        let file: PatchFile =
            toml::from_str(text).map_err(|e| PatchError::Syntax(e.message().to_string()))?;

        let mut params = Params::default();
        let fields: [(ParamId, &'static str, Option<f32>); 6] = [
            (ParamId::Waveform, "oscillator.waveform", file.oscillator.waveform.as_ref().map(WaveformName::value)),
            (ParamId::Attack, "envelope.attack", file.envelope.attack),
            (ParamId::Decay, "envelope.decay", file.envelope.decay),
            (ParamId::Sustain, "envelope.sustain", file.envelope.sustain),
            (ParamId::Release, "envelope.release", file.envelope.release),
            (ParamId::Volume, "output.volume", file.output.volume),
        ];
        for (id, field, value) in fields {
            let Some(value) = value else { continue };
            let spec = id.spec();
            if !(spec.min..=spec.max).contains(&value) {
                return Err(PatchError::OutOfRange { field, value, min: spec.min, max: spec.max });
            }
            params.set(id, value);
        }
        Ok(Patch { name: file.name.unwrap_or_else(|| "Init".into()), params })
    }

    /// Texto TOML con comentarios, listo para guardar y editar a mano.
    pub fn to_toml(&self) -> String {
        let p = &self.params;
        let name = toml::Value::String(self.name.clone());
        format!(
            "name = {name}\n\n\
             [oscillator]\n\
             waveform = \"{}\"  # sine, saw, square o triangle\n\n\
             [envelope]\n\
             attack = {}   # segundos\n\
             decay = {}\n\
             sustain = {}  # 0.0 a 1.0\n\
             release = {}\n\n\
             [output]\n\
             volume = {}   # 0.0 a 1.0\n",
            waveform_text(p.get(ParamId::Waveform)),
            p.get(ParamId::Attack),
            p.get(ParamId::Decay),
            p.get(ParamId::Sustain),
            p.get(ParamId::Release),
            p.get(ParamId::Volume),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_the_default_patch() {
        assert_eq!(Patch::from_toml("").unwrap(), Patch::default());
    }

    #[test]
    fn parses_a_full_patch() {
        let patch = Patch::from_toml(
            r#"
            name = "Pad01"
            [oscillator]
            waveform = "triangle"
            [envelope]
            attack = 0.4
            decay = 0.5
            sustain = 0.8
            release = 1.2
            [output]
            volume = 0.6
            "#,
        )
        .unwrap();
        assert_eq!(patch.name, "Pad01");
        assert_eq!(patch.params.get(ParamId::Waveform), 3.0);
        assert!((patch.params.get(ParamId::Attack) - 0.4).abs() < 1e-6);
        assert!((patch.params.get(ParamId::Release) - 1.2).abs() < 1e-6);
        assert!((patch.params.get(ParamId::Volume) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn missing_values_keep_their_defaults() {
        let patch = Patch::from_toml("[envelope]\nrelease = 2.0\n").unwrap();
        let defaults = Params::default();
        assert_eq!(patch.params.get(ParamId::Release), 2.0);
        assert_eq!(patch.params.get(ParamId::Attack), defaults.get(ParamId::Attack));
        assert_eq!(patch.params.get(ParamId::Volume), defaults.get(ParamId::Volume));
    }

    #[test]
    fn unknown_keys_are_rejected_so_typos_show_up() {
        assert!(matches!(Patch::from_toml("[envelope]\natack = 0.1\n"), Err(PatchError::Syntax(_))));
        assert!(matches!(Patch::from_toml("[filter]\ncutoff = 1\n"), Err(PatchError::Syntax(_))));
    }

    #[test]
    fn out_of_range_values_name_the_field() {
        let err = Patch::from_toml("[envelope]\nsustain = 1.5\n").unwrap_err();
        assert!(matches!(err, PatchError::OutOfRange { field: "envelope.sustain", .. }));
        assert!(err.to_string().contains("envelope.sustain"));
        assert!(Patch::from_toml("[envelope]\nattack = 0.0\n").is_err());
        assert!(Patch::from_toml("[output]\nvolume = nan\n").is_err());
    }

    #[test]
    fn bad_waveform_and_bad_syntax_are_errors() {
        assert!(Patch::from_toml("[oscillator]\nwaveform = \"noise\"\n").is_err());
        assert!(Patch::from_toml("[oscillator]\nwaveform = 1\n").is_err());
        assert!(Patch::from_toml("name = \n").is_err());
        assert!(Patch::from_toml("[envelope\n").is_err());
    }

    #[test]
    fn written_patches_read_back_identically() {
        for waveform in 0..=3 {
            let mut patch = Patch { name: "Prueba \"con\" comillas".into(), params: Params::default() };
            patch.params.set(ParamId::Waveform, waveform as f32);
            patch.params.set(ParamId::Attack, 0.013);
            patch.params.set(ParamId::Decay, 1.5);
            patch.params.set(ParamId::Sustain, 0.35);
            patch.params.set(ParamId::Release, 4.25);
            patch.params.set(ParamId::Volume, 0.8);
            let text = patch.to_toml();
            assert_eq!(Patch::from_toml(&text).unwrap(), patch, "texto:\n{text}");
        }
    }

    #[test]
    fn written_text_is_human_friendly() {
        let text = Patch::default().to_toml();
        assert!(text.contains("waveform = \"saw\""));
        assert!(text.contains("attack = 0.01 "));
        assert!(!text.contains("0.0099999"), "los floats deben salir cortos:\n{text}");
    }
}
