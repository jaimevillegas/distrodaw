//! Eventos que entran al motor. Todos llevan su posición en frames dentro del
//! buffer que se va a renderizar, lo que mantiene el tiempo del secuenciador.

use crate::params::ParamId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventKind {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    SetParam { id: ParamId, value: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    /// Frame dentro del buffer (0 = primera muestra). Un valor mayor que el
    /// buffer se aplica al final de este.
    pub frame: u32,
    pub kind: EventKind,
}

impl Event {
    pub fn new(frame: u32, kind: EventKind) -> Self {
        Self { frame, kind }
    }

    pub fn note_on(frame: u32, note: u8, velocity: u8) -> Self {
        Self::new(frame, EventKind::NoteOn { note, velocity })
    }

    pub fn note_off(frame: u32, note: u8) -> Self {
        Self::new(frame, EventKind::NoteOff { note })
    }

    pub fn set_param(frame: u32, id: ParamId, value: f32) -> Self {
        Self::new(frame, EventKind::SetParam { id, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_build_expected_kinds() {
        assert_eq!(
            Event::note_on(3, 60, 100).kind,
            EventKind::NoteOn { note: 60, velocity: 100 }
        );
        assert_eq!(Event::note_off(4, 60).frame, 4);
        assert_eq!(
            Event::set_param(0, ParamId::Volume, 0.5).kind,
            EventKind::SetParam { id: ParamId::Volume, value: 0.5 }
        );
    }

    #[test]
    fn event_is_small_and_copy() {
        // Viaja por una cola sin bloqueo: debe ser barato de copiar.
        assert!(std::mem::size_of::<Event>() <= 16);
        let e = Event::note_on(0, 60, 100);
        let f = e;
        assert_eq!(e, f);
    }
}
