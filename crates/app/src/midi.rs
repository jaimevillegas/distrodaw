//! Entrada MIDI: listado de puertos e interpretación de mensajes.

use anyhow::{anyhow, bail, Result};
use distrodaw_engine::{EventKind, ParamId};
use midir::{MidiInput, MidiInputPort};

/// Mensaje MIDI de canal ya interpretado. Se ignora el canal (modo omni).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Midi {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    ControlChange { controller: u8, value: u8 },
}

/// Interpreta los bytes crudos de un mensaje. Devuelve `None` para lo que el
/// DAW aún no usa (reloj, pitch bend, mensajes de sistema, etc.).
pub fn parse(bytes: &[u8]) -> Option<Midi> {
    let (&status, data) = bytes.split_first()?;
    match (status & 0xF0, data) {
        (0x80, &[note, _velocity, ..]) => Some(Midi::NoteOff { note: note & 0x7F }),
        // Note On con velocidad 0 equivale a Note Off.
        (0x90, &[note, 0, ..]) => Some(Midi::NoteOff { note: note & 0x7F }),
        (0x90, &[note, velocity, ..]) => {
            Some(Midi::NoteOn { note: note & 0x7F, velocity: velocity & 0x7F })
        }
        (0xB0, &[controller, value, ..]) => {
            Some(Midi::ControlChange { controller: controller & 0x7F, value: value & 0x7F })
        }
        _ => None,
    }
}

/// CC estándar «All Notes Off».
const ALL_NOTES_OFF_CC: u8 = 123;

/// Traduce un mensaje MIDI al evento que entiende el motor.
pub fn to_event(msg: Midi) -> Option<EventKind> {
    match msg {
        Midi::NoteOn { note, velocity } => Some(EventKind::NoteOn { note, velocity }),
        Midi::NoteOff { note } => Some(EventKind::NoteOff { note }),
        Midi::ControlChange { controller: ALL_NOTES_OFF_CC, .. } => Some(EventKind::AllNotesOff),
        Midi::ControlChange { controller, value } => {
            let (id, value) = cc_to_param(controller, value)?;
            Some(EventKind::SetParam { id, value })
        }
    }
}

/// Mapa fijo de controladores MIDI a parámetros (estándar «sound controllers»
/// siempre que existe uno): 7 volumen, 70 forma de onda, 72 release, 73 attack,
/// 75 decay, 79 sustain. El reparto del rango lo define `ParamKind`.
pub fn cc_to_param(controller: u8, value: u8) -> Option<(ParamId, f32)> {
    let id = match controller {
        7 => ParamId::Volume,
        70 => ParamId::Waveform,
        72 => ParamId::Release,
        73 => ParamId::Attack,
        75 => ParamId::Decay,
        79 => ParamId::Sustain,
        _ => return None,
    };
    let norm = f32::from(value.min(127)) / 127.0;
    Some((id, id.spec().from_normalized(norm)))
}

/// Elige el puerto cuyo nombre contiene `filter`; sin filtro, el primero que
/// no sea el puerto virtual «Midi Through».
pub fn pick_port(input: &MidiInput, filter: Option<&str>) -> Result<MidiInputPort> {
    let filter = filter.map(str::to_lowercase);
    for port in input.ports() {
        let name = input.port_name(&port).map_err(|e| anyhow!("{e}"))?.to_lowercase();
        let wanted = match &filter {
            Some(f) => name.contains(f.as_str()),
            None => !name.contains("midi through"),
        };
        if wanted {
            return Ok(port);
        }
    }
    match filter {
        Some(f) => bail!("ningún puerto MIDI coincide con {f:?}; usa `midi-ports`"),
        None => bail!("no hay teclado MIDI conectado; usa `midi-ports` para ver los puertos"),
    }
}

pub fn list_ports() -> Result<()> {
    let input = MidiInput::new("distrodaw")?;
    let ports = input.ports();
    if ports.is_empty() {
        println!("No hay puertos de entrada MIDI.");
    }
    for port in &ports {
        println!("{}", input.port_name(port).map_err(|e| anyhow!("{e}"))?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_note_on_and_off() {
        assert_eq!(parse(&[0x90, 60, 100]), Some(Midi::NoteOn { note: 60, velocity: 100 }));
        assert_eq!(parse(&[0x80, 60, 64]), Some(Midi::NoteOff { note: 60 }));
    }

    #[test]
    fn note_on_with_zero_velocity_is_note_off() {
        assert_eq!(parse(&[0x93, 72, 0]), Some(Midi::NoteOff { note: 72 }));
    }

    #[test]
    fn ignores_the_channel() {
        assert_eq!(parse(&[0x9F, 40, 1]), Some(Midi::NoteOn { note: 40, velocity: 1 }));
        assert_eq!(parse(&[0x85, 40, 0]), Some(Midi::NoteOff { note: 40 }));
    }

    #[test]
    fn parses_control_change() {
        assert_eq!(
            parse(&[0xB0, 73, 127]),
            Some(Midi::ControlChange { controller: 73, value: 127 })
        );
    }

    #[test]
    fn notes_become_engine_events() {
        assert_eq!(
            to_event(Midi::NoteOn { note: 60, velocity: 90 }),
            Some(EventKind::NoteOn { note: 60, velocity: 90 })
        );
        assert_eq!(to_event(Midi::NoteOff { note: 60 }), Some(EventKind::NoteOff { note: 60 }));
    }

    #[test]
    fn cc_maps_to_parameter_ranges() {
        assert_eq!(cc_to_param(7, 0), Some((ParamId::Volume, 0.0)));
        assert_eq!(cc_to_param(7, 127), Some((ParamId::Volume, 1.0)));
        assert_eq!(cc_to_param(79, 127), Some((ParamId::Sustain, 1.0)));
        assert_eq!(cc_to_param(70, 127), Some((ParamId::Waveform, 3.0)));
        assert_eq!(cc_to_param(1, 64), None);
    }

    #[test]
    fn time_parameters_use_a_quadratic_curve() {
        let (_, half) = cc_to_param(73, 64).unwrap();
        let spec = ParamId::Attack.spec();
        // A media escala el ataque es mucho más corto que a mitad de rango lineal.
        assert!(half < spec.min + 0.3 * (spec.max - spec.min));
        let (_, max) = cc_to_param(73, 127).unwrap();
        assert!((max - spec.max).abs() < 1e-4);
        let (_, min) = cc_to_param(72, 0).unwrap();
        assert_eq!(min, ParamId::Release.spec().min);
    }

    #[test]
    fn cc_123_releases_all_notes() {
        assert_eq!(
            to_event(Midi::ControlChange { controller: 123, value: 0 }),
            Some(EventKind::AllNotesOff)
        );
    }

    #[test]
    fn control_change_becomes_set_param() {
        assert_eq!(
            to_event(Midi::ControlChange { controller: 7, value: 127 }),
            Some(EventKind::SetParam { id: ParamId::Volume, value: 1.0 })
        );
        assert_eq!(to_event(Midi::ControlChange { controller: 1, value: 10 }), None);
    }

    #[test]
    fn ignores_unsupported_or_truncated_messages() {
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[0x90, 60]), None);
        assert_eq!(parse(&[0xF8]), None); // reloj MIDI
        assert_eq!(parse(&[0xE0, 0, 64]), None); // pitch bend
        assert_eq!(parse(&[0xFE]), None); // active sensing
    }
}
