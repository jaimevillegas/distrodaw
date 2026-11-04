//! Sinte polifónico de 8 voces. Sin reservas de memoria ni locks: es seguro
//! llamar a `render` desde el hilo de audio.

use crate::env::EnvCoefs;
use crate::event::{Event, EventKind};
use crate::params::{ParamId, Params};
use crate::voice::{Voice, Waveform};

pub const VOICES: usize = 8;

/// Margen para que la suma de varias voces no sature.
const HEADROOM: f32 = 0.25;

#[derive(Debug, Clone)]
pub struct Synth {
    sample_rate: f32,
    params: Params,
    coefs: EnvCoefs,
    voices: [Voice; VOICES],
    /// Contador creciente para saber qué voz es la más antigua.
    counter: u64,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
        let params = Params::default();
        let coefs = Self::coefs_for(sample_rate, &params);
        Self { sample_rate, params, coefs, voices: [Voice::default(); VOICES], counter: 0 }
    }

    pub fn params(&self) -> &Params {
        &self.params
    }

    /// Número de voces sonando, incluidas las que están en relajación.
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active()).count()
    }

    fn coefs_for(sample_rate: f32, p: &Params) -> EnvCoefs {
        EnvCoefs::new(
            sample_rate,
            p.get(ParamId::Attack),
            p.get(ParamId::Decay),
            p.get(ParamId::Sustain),
            p.get(ParamId::Release),
        )
    }

    /// Rellena `out` (mono) aplicando `events` en su frame. Los eventos deben
    /// venir ordenados por `frame`; uno que llegue antes de la posición actual
    /// se aplica de inmediato.
    pub fn render(&mut self, events: &[Event], out: &mut [f32]) {
        out.fill(0.0);
        let mut pos = 0;
        for event in events {
            let at = (event.frame as usize).min(out.len());
            if at > pos {
                self.render_voices(&mut out[pos..at]);
                pos = at;
            }
            self.apply(event.kind);
        }
        self.render_voices(&mut out[pos..]);
    }

    fn render_voices(&mut self, out: &mut [f32]) {
        let waveform = Waveform::from_param(self.params.get(ParamId::Waveform));
        for voice in self.voices.iter_mut().filter(|v| v.is_active()) {
            voice.render(waveform, &self.coefs, out);
        }
        let gain = self.params.get(ParamId::Volume) * HEADROOM;
        for sample in out.iter_mut() {
            *sample = (*sample * gain).clamp(-1.0, 1.0);
        }
    }

    fn apply(&mut self, kind: EventKind) {
        match kind {
            EventKind::NoteOn { note, velocity: 0 } => self.note_off(note),
            EventKind::NoteOn { note, velocity } => self.note_on(note, velocity),
            EventKind::NoteOff { note } => self.note_off(note),
            EventKind::AllNotesOff => {
                for voice in self.voices.iter_mut().filter(|v| v.gate()) {
                    voice.note_off();
                }
            }
            EventKind::SetParam { id, value } => {
                self.params.set(id, value);
                if matches!(id, ParamId::Attack | ParamId::Decay | ParamId::Sustain | ParamId::Release) {
                    self.coefs = Self::coefs_for(self.sample_rate, &self.params);
                }
            }
        }
    }

    fn note_on(&mut self, note: u8, velocity: u8) {
        self.counter += 1;
        let index = self.voice_for(note);
        self.voices[index].note_on(self.sample_rate, note, velocity, self.counter);
    }

    fn note_off(&mut self, note: u8) {
        for voice in self.voices.iter_mut().filter(|v| v.gate() && v.note() == note) {
            voice.note_off();
        }
    }

    /// Reutiliza la voz de la misma nota; si no, una libre; si no, roba la
    /// más antigua, prefiriendo las que ya están soltando la nota.
    fn voice_for(&self, note: u8) -> usize {
        self.voices
            .iter()
            .position(|v| v.gate() && v.note() == note)
            .or_else(|| self.voices.iter().position(|v| !v.is_active()))
            .unwrap_or_else(|| {
                (0..VOICES)
                    .min_by_key(|&i| (self.voices[i].gate(), self.voices[i].age()))
                    .unwrap_or(0)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn render(synth: &mut Synth, events: &[Event], frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames];
        synth.render(events, &mut out);
        out
    }

    fn peak(buf: &[f32]) -> f32 {
        buf.iter().fold(0.0, |m, s| m.max(s.abs()))
    }

    #[test]
    fn silent_without_notes() {
        let mut s = Synth::new(SR);
        assert_eq!(peak(&render(&mut s, &[], 512)), 0.0);
        assert_eq!(s.active_voices(), 0);
    }

    #[test]
    fn note_on_makes_sound_within_range() {
        let mut s = Synth::new(SR);
        let out = render(&mut s, &[Event::note_on(0, 60, 100)], 4_800);
        assert!(peak(&out) > 0.05);
        assert!(peak(&out) <= 1.0);
        assert_eq!(s.active_voices(), 1);
    }

    #[test]
    fn event_frame_places_the_note_inside_the_buffer() {
        let mut s = Synth::new(SR);
        let out = render(&mut s, &[Event::note_on(100, 69, 127)], 1_000);
        assert!(out[..100].iter().all(|&v| v == 0.0));
        assert!(peak(&out[100..]) > 0.0);
    }

    #[test]
    fn note_off_releases_to_silence() {
        let mut s = Synth::new(SR);
        s.apply(EventKind::SetParam { id: ParamId::Release, value: 0.05 });
        render(&mut s, &[Event::note_on(0, 60, 100)], 4_800);
        render(&mut s, &[Event::note_off(0, 60)], 480);
        assert_eq!(s.active_voices(), 1, "sigue en relajación");
        let tail = render(&mut s, &[], 48_000);
        assert_eq!(s.active_voices(), 0);
        assert_eq!(peak(&tail[40_000..]), 0.0);
    }

    #[test]
    fn velocity_zero_acts_as_note_off() {
        let mut s = Synth::new(SR);
        render(&mut s, &[Event::note_on(0, 60, 100)], 480);
        render(&mut s, &[Event::note_on(0, 60, 0)], 16);
        assert!(!s.voices.iter().any(|v| v.gate()));
    }

    #[test]
    fn same_note_reuses_its_voice() {
        let mut s = Synth::new(SR);
        render(&mut s, &[Event::note_on(0, 60, 100), Event::note_on(10, 60, 100)], 480);
        assert_eq!(s.active_voices(), 1);
    }

    #[test]
    fn ninth_note_steals_the_oldest_voice() {
        let mut s = Synth::new(SR);
        let notes: Vec<Event> = (0..9).map(|i| Event::note_on(i, 60 + i as u8, 100)).collect();
        render(&mut s, &notes, 480);
        assert_eq!(s.active_voices(), VOICES);
        let held: Vec<u8> = s.voices.iter().filter(|v| v.gate()).map(|v| v.note()).collect();
        assert!(!held.contains(&60), "la nota más antigua debió ceder su voz");
        assert!(held.contains(&68));
    }

    #[test]
    fn stealing_prefers_voices_already_releasing() {
        let mut s = Synth::new(SR);
        let notes: Vec<Event> = (0..8).map(|i| Event::note_on(i, 60 + i as u8, 100)).collect();
        render(&mut s, &notes, 480);
        // La nota 65 se suelta pero sigue sonando; la 60 es la más antigua.
        render(&mut s, &[Event::note_off(0, 65), Event::note_on(1, 80, 100)], 16);
        let held: Vec<u8> = s.voices.iter().filter(|v| v.gate()).map(|v| v.note()).collect();
        assert!(held.contains(&60), "no debía robarse la nota 60");
        assert!(held.contains(&80));
    }

    #[test]
    fn set_param_events_change_the_sound() {
        let mut s = Synth::new(SR);
        render(&mut s, &[Event::set_param(0, ParamId::Volume, 0.0), Event::note_on(0, 60, 127)], 2_000);
        let out = render(&mut s, &[], 2_000);
        assert_eq!(peak(&out), 0.0);

        render(&mut s, &[Event::set_param(0, ParamId::Volume, 1.0)], 16);
        let out = render(&mut s, &[], 2_000);
        assert!(peak(&out) > 0.05);
        assert_eq!(s.params().get(ParamId::Volume), 1.0);
    }

    #[test]
    fn mix_never_exceeds_full_scale() {
        let mut s = Synth::new(SR);
        s.apply(EventKind::SetParam { id: ParamId::Volume, value: 1.0 });
        s.apply(EventKind::SetParam { id: ParamId::Waveform, value: 2.0 });
        let notes: Vec<Event> = (0..8).map(|i| Event::note_on(0, 48 + i, 127)).collect();
        let out = render(&mut s, &notes, 9_600);
        assert!(peak(&out) <= 1.0);
    }

    #[test]
    fn all_notes_off_releases_every_held_note() {
        let mut s = Synth::new(SR);
        let notes: Vec<Event> = (0..5).map(|i| Event::note_on(0, 60 + i, 100)).collect();
        render(&mut s, &notes, 480);
        assert!(s.voices.iter().filter(|v| v.gate()).count() == 5);
        render(&mut s, &[Event::all_notes_off(0)], 16);
        assert!(!s.voices.iter().any(|v| v.gate()));
        assert_eq!(s.active_voices(), 5, "siguen en relajación, sin cortes secos");
    }
}
