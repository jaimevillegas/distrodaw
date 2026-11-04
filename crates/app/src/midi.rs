//! Entrada MIDI: listado de puertos e interpretación de mensajes.

use anyhow::{anyhow, Result};
use midir::MidiInput;

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
    fn ignores_unsupported_or_truncated_messages() {
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[0x90, 60]), None);
        assert_eq!(parse(&[0xF8]), None); // reloj MIDI
        assert_eq!(parse(&[0xE0, 0, 64]), None); // pitch bend
        assert_eq!(parse(&[0xFE]), None); // active sensing
    }
}
