//! Textos para mostrar valores: notas, parámetros y barras.

use distrodaw_engine::ParamId;

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Nombre de una nota MIDI con su octava; 60 es C4.
pub fn note_name(note: u8) -> String {
    let octave = i16::from(note / 12) - 1;
    format!("{}{octave}", NOTE_NAMES[usize::from(note % 12)])
}

pub fn waveform_name(value: f32) -> &'static str {
    match value.round() as i32 {
        0 => "sine",
        1 => "saw",
        2 => "square",
        _ => "triangle",
    }
}

/// Valor de un parámetro con su unidad.
pub fn param_value(id: ParamId, value: f32) -> String {
    match id {
        ParamId::Waveform => waveform_name(value).to_string(),
        ParamId::Attack | ParamId::Decay | ParamId::Release => {
            if value < 1.0 {
                format!("{:.0} ms", value * 1000.0)
            } else {
                format!("{value:.2} s")
            }
        }
        ParamId::Sustain | ParamId::Volume => format!("{value:.2}"),
    }
}

/// Barra de `width` celdas, llena en la fracción `fraction` (0.0 a 1.0).
pub fn bar(fraction: f32, width: usize) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!("{}{}", "▓".repeat(filled), "░".repeat(width - filled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_names_use_c4_as_middle_c() {
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(69), "A4");
        assert_eq!(note_name(61), "C#4");
        assert_eq!(note_name(0), "C-1");
        assert_eq!(note_name(127), "G9");
    }

    #[test]
    fn parameter_values_have_units() {
        assert_eq!(param_value(ParamId::Attack, 0.01), "10 ms");
        assert_eq!(param_value(ParamId::Release, 2.5), "2.50 s");
        assert_eq!(param_value(ParamId::Sustain, 0.7), "0.70");
        assert_eq!(param_value(ParamId::Waveform, 1.0), "saw");
    }

    #[test]
    fn bars_fill_proportionally() {
        assert_eq!(bar(0.0, 4), "░░░░");
        assert_eq!(bar(0.5, 4), "▓▓░░");
        assert_eq!(bar(1.0, 4), "▓▓▓▓");
        assert_eq!(bar(7.0, 4), "▓▓▓▓");
        assert_eq!(bar(-1.0, 4), "░░░░");
    }
}
