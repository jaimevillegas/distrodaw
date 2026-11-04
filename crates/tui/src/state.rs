//! Estado de la interfaz y acciones que lo modifican. No depende del
//! terminal ni de la entrada: teclado, joystick o MIDI producen las mismas
//! [`Action`] y se pueden probar sin pantalla.

use distrodaw_engine::{ParamId, ParamKind, PARAM_COUNT, SPECS};

/// Cuántas notas recientes se recuerdan para la pantalla Play.
pub const MAX_RECENT_NOTES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Play,
    Synth,
}

impl Screen {
    pub const ALL: [Screen; 2] = [Screen::Play, Screen::Synth];

    pub fn next(self) -> Screen {
        match self {
            Screen::Play => Screen::Synth,
            Screen::Synth => Screen::Play,
        }
    }
}

/// Tamaño del ajuste de un parámetro, como fracción de su rango.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Fine,
    Normal,
    Coarse,
}

impl Step {
    fn size(self) -> f32 {
        match self {
            Step::Fine => 1.0 / 250.0,
            Step::Normal => 1.0 / 50.0,
            Step::Coarse => 1.0 / 10.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Screen(Screen),
    NextScreen,
    /// Parámetro anterior en la pantalla Synth.
    Up,
    /// Parámetro siguiente en la pantalla Synth.
    Down,
    /// Sube (`direction` > 0) o baja el parámetro seleccionado.
    Adjust { direction: i8, step: Step },
    Quit,
}

/// Cambio de parámetro que la interfaz pide al motor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edit {
    pub id: ParamId,
    pub value: f32,
}

#[derive(Debug, Clone)]
pub struct UiState {
    pub screen: Screen,
    /// Índice del parámetro seleccionado en la pantalla Synth.
    pub selected: usize,
    pub params: [f32; PARAM_COUNT],
    /// Nivel de salida (0.0 a 1.0) con caída suave para el medidor.
    pub peak: f32,
    pub voices: usize,
    /// Notas MIDI recientes, la más nueva al final.
    pub recent_notes: Vec<u8>,
    /// Fracción media del tiempo de buffer que usa el callback (1.0 = 100 %).
    pub cpu_load: f32,
    pub misses: u64,
    pub errors: u64,
    pub midi_port: String,
    pub audio_info: String,
    pub patch: String,
    pub quit: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            screen: Screen::Play,
            selected: 0,
            params: SPECS.map(|s| s.default),
            peak: 0.0,
            voices: 0,
            recent_notes: Vec::with_capacity(MAX_RECENT_NOTES),
            cpu_load: 0.0,
            misses: 0,
            errors: 0,
            midi_port: String::new(),
            audio_info: String::new(),
            patch: "Init".into(),
            quit: false,
        }
    }
}

impl UiState {
    pub fn selected_id(&self) -> ParamId {
        ParamId::from_index(self.selected).unwrap_or(ParamId::Waveform)
    }

    /// Aplica una acción. Devuelve el cambio de parámetro que haya que enviar al motor.
    pub fn apply(&mut self, action: Action) -> Option<Edit> {
        match action {
            Action::Screen(screen) => self.screen = screen,
            Action::NextScreen => self.screen = self.screen.next(),
            Action::Quit => self.quit = true,
            Action::Up if self.screen == Screen::Synth => {
                self.selected = (self.selected + PARAM_COUNT - 1) % PARAM_COUNT;
            }
            Action::Down if self.screen == Screen::Synth => {
                self.selected = (self.selected + 1) % PARAM_COUNT;
            }
            Action::Adjust { direction, step } if self.screen == Screen::Synth => {
                return self.adjust(direction, step);
            }
            Action::Up | Action::Down | Action::Adjust { .. } => {}
        }
        None
    }

    fn adjust(&mut self, direction: i8, step: Step) -> Option<Edit> {
        let id = self.selected_id();
        let spec = id.spec();
        let current = self.params[self.selected];
        let sign = f32::from(direction.signum());
        let value = match spec.kind {
            ParamKind::Stepped => (current + sign).clamp(spec.min, spec.max),
            ParamKind::Linear | ParamKind::Time => {
                spec.from_normalized(spec.to_normalized(current) + sign * step.size())
            }
        };
        if value == current {
            return None;
        }
        self.params[self.selected] = value;
        Some(Edit { id, value })
    }

    pub fn note_on(&mut self, note: u8) {
        if self.recent_notes.len() == MAX_RECENT_NOTES {
            self.recent_notes.remove(0);
        }
        self.recent_notes.push(note);
    }

    /// Actualiza el medidor con el pico `raw` medido desde el cuadro anterior.
    pub fn set_peak(&mut self, raw: f32) {
        self.peak = raw.clamp(0.0, 1.0).max(self.peak * 0.9);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_state() -> UiState {
        let mut s = UiState::default();
        s.apply(Action::Screen(Screen::Synth));
        s
    }

    #[test]
    fn screens_switch_directly_and_cyclically() {
        let mut s = UiState::default();
        assert_eq!(s.screen, Screen::Play);
        s.apply(Action::NextScreen);
        assert_eq!(s.screen, Screen::Synth);
        s.apply(Action::NextScreen);
        assert_eq!(s.screen, Screen::Play);
        s.apply(Action::Screen(Screen::Synth));
        assert_eq!(s.screen, Screen::Synth);
    }

    #[test]
    fn selection_wraps_around() {
        let mut s = synth_state();
        s.apply(Action::Up);
        assert_eq!(s.selected, PARAM_COUNT - 1);
        s.apply(Action::Down);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn selection_and_edits_only_work_on_synth_screen() {
        let mut s = UiState::default();
        s.apply(Action::Down);
        assert_eq!(s.selected, 0);
        assert_eq!(s.apply(Action::Adjust { direction: 1, step: Step::Normal }), None);
    }

    #[test]
    fn adjusting_a_continuous_parameter_returns_an_edit() {
        let mut s = synth_state();
        s.selected = ParamId::Sustain as usize;
        let before = s.params[s.selected];
        let edit = s.apply(Action::Adjust { direction: 1, step: Step::Normal }).unwrap();
        assert_eq!(edit.id, ParamId::Sustain);
        assert!((edit.value - (before + 0.02)).abs() < 1e-4);
        assert_eq!(s.params[s.selected], edit.value);
    }

    #[test]
    fn step_sizes_are_ordered() {
        let delta = |step| {
            let mut s = synth_state();
            s.selected = ParamId::Volume as usize;
            let before = s.params[s.selected];
            s.apply(Action::Adjust { direction: 1, step }).unwrap().value - before
        };
        assert!(delta(Step::Fine) < delta(Step::Normal));
        assert!(delta(Step::Normal) < delta(Step::Coarse));
    }

    #[test]
    fn waveform_moves_one_value_per_press() {
        let mut s = synth_state();
        s.selected = ParamId::Waveform as usize;
        let before = s.params[s.selected];
        let edit = s.apply(Action::Adjust { direction: 1, step: Step::Fine }).unwrap();
        assert_eq!(edit.value, before + 1.0);
    }

    #[test]
    fn adjusting_at_a_limit_sends_nothing() {
        let mut s = synth_state();
        s.selected = ParamId::Volume as usize;
        s.params[s.selected] = 1.0;
        assert_eq!(s.apply(Action::Adjust { direction: 1, step: Step::Coarse }), None);
        s.selected = ParamId::Waveform as usize;
        s.params[s.selected] = 0.0;
        assert_eq!(s.apply(Action::Adjust { direction: -1, step: Step::Normal }), None);
    }

    #[test]
    fn time_parameters_never_leave_their_range() {
        let mut s = synth_state();
        s.selected = ParamId::Attack as usize;
        for _ in 0..200 {
            s.apply(Action::Adjust { direction: -1, step: Step::Coarse });
        }
        assert_eq!(s.params[s.selected], ParamId::Attack.spec().min);
        for _ in 0..200 {
            s.apply(Action::Adjust { direction: 1, step: Step::Coarse });
        }
        assert_eq!(s.params[s.selected], ParamId::Attack.spec().max);
    }

    #[test]
    fn quit_sets_the_flag() {
        let mut s = UiState::default();
        s.apply(Action::Quit);
        assert!(s.quit);
    }

    #[test]
    fn recent_notes_are_capped_and_keep_the_newest() {
        let mut s = UiState::default();
        for n in 0..(MAX_RECENT_NOTES as u8 + 3) {
            s.note_on(40 + n);
        }
        assert_eq!(s.recent_notes.len(), MAX_RECENT_NOTES);
        assert_eq!(*s.recent_notes.last().unwrap(), 40 + MAX_RECENT_NOTES as u8 + 2);
    }

    #[test]
    fn peak_rises_instantly_and_falls_smoothly() {
        let mut s = UiState::default();
        s.set_peak(0.8);
        assert_eq!(s.peak, 0.8);
        s.set_peak(0.0);
        assert!(s.peak < 0.8 && s.peak > 0.5);
        s.set_peak(5.0);
        assert_eq!(s.peak, 1.0);
    }
}
