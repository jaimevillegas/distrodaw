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
    Files,
}

impl Screen {
    pub const ALL: [Screen; 3] = [Screen::Play, Screen::Synth, Screen::Files];

    pub fn next(self) -> Screen {
        match self {
            Screen::Play => Screen::Synth,
            Screen::Synth => Screen::Files,
            Screen::Files => Screen::Play,
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
    /// Acepta la selección (en Files, carga el patch).
    Confirm,
    /// Guarda sobre el patch seleccionado (en Files).
    Save,
    /// Guarda como un patch nuevo (en Files).
    SaveNew,
    Quit,
}

/// Operación con archivos que la interfaz pide a la app (la interfaz no toca el disco).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRequest {
    /// Cargar el archivo número `n` de [`UiState::files`].
    Load(usize),
    /// Guardar los parámetros actuales sobre el archivo número `n`.
    Save(usize),
    /// Guardar los parámetros actuales en un archivo nuevo.
    SaveNew,
}

/// Mensaje para el usuario, p. ej. el resultado de cargar un patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub text: String,
    pub error: bool,
}

/// Cuadros que se mantiene un mensaje de éxito (~5 s a 30 cuadros por segundo).
const STATUS_FRAMES: u16 = 150;

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
    /// Directorio de patches que se muestra en la pantalla Files.
    pub patch_dir: String,
    /// Nombres de archivo de los patches disponibles.
    pub files: Vec<String>,
    pub file_cursor: usize,
    /// Archivo del que viene el patch actual, si viene de uno.
    pub loaded_file: Option<String>,
    pub status: Option<Status>,
    status_frames: u16,
    /// Operación pendiente que la app debe atender y vaciar con `take()`.
    pub request: Option<FileRequest>,
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
            patch_dir: String::new(),
            files: Vec::new(),
            file_cursor: 0,
            loaded_file: None,
            status: None,
            status_frames: 0,
            request: None,
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
            Action::Up if self.screen == Screen::Files && !self.files.is_empty() => {
                self.file_cursor = (self.file_cursor + self.files.len() - 1) % self.files.len();
            }
            Action::Down if self.screen == Screen::Files && !self.files.is_empty() => {
                self.file_cursor = (self.file_cursor + 1) % self.files.len();
            }
            Action::Confirm if self.screen == Screen::Files && !self.files.is_empty() => {
                self.request = Some(FileRequest::Load(self.file_cursor));
            }
            Action::Save if self.screen == Screen::Files && !self.files.is_empty() => {
                self.request = Some(FileRequest::Save(self.file_cursor));
            }
            Action::SaveNew if self.screen == Screen::Files => {
                self.request = Some(FileRequest::SaveNew);
            }
            Action::Adjust { direction, step } if self.screen == Screen::Synth => {
                return self.adjust(direction, step);
            }
            Action::Up
            | Action::Down
            | Action::Adjust { .. }
            | Action::Confirm
            | Action::Save
            | Action::SaveNew => {}
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

    /// Sustituye la lista de patches, conservando el cursor dentro de rango.
    pub fn set_files(&mut self, files: Vec<String>) {
        self.files = files;
        self.file_cursor = self.file_cursor.min(self.files.len().saturating_sub(1));
    }

    /// Muestra un mensaje. Los de éxito caducan solos; los errores se quedan hasta el siguiente mensaje.
    pub fn set_status(&mut self, text: impl Into<String>, error: bool) {
        self.status = Some(Status { text: text.into(), error });
        self.status_frames = if error { u16::MAX } else { STATUS_FRAMES };
    }

    /// Avanza un cuadro: hace caducar los mensajes de éxito.
    pub fn tick(&mut self) {
        match self.status_frames {
            0 => self.status = None,
            u16::MAX => {}
            n => self.status_frames = n - 1,
        }
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
        assert_eq!(s.screen, Screen::Files);
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

    fn files_state() -> UiState {
        let mut s = UiState::default();
        s.apply(Action::Screen(Screen::Files));
        s.set_files(vec!["a.toml".into(), "b.toml".into(), "c.toml".into()]);
        s
    }

    #[test]
    fn file_cursor_wraps_and_requests_the_right_file() {
        let mut s = files_state();
        s.apply(Action::Up);
        assert_eq!(s.file_cursor, 2);
        s.apply(Action::Down);
        s.apply(Action::Down);
        assert_eq!(s.file_cursor, 1);

        assert_eq!(s.apply(Action::Confirm), None);
        assert_eq!(s.request.take(), Some(FileRequest::Load(1)));
        s.apply(Action::Save);
        assert_eq!(s.request.take(), Some(FileRequest::Save(1)));
        s.apply(Action::SaveNew);
        assert_eq!(s.request.take(), Some(FileRequest::SaveNew));
    }

    #[test]
    fn file_actions_do_nothing_outside_the_files_screen_or_without_files() {
        let mut s = UiState::default();
        s.set_files(vec!["a.toml".into()]);
        for action in [Action::Confirm, Action::Save, Action::SaveNew] {
            s.apply(action);
            assert_eq!(s.request, None, "{action:?} en Play");
        }
        let mut empty = UiState::default();
        empty.apply(Action::Screen(Screen::Files));
        empty.apply(Action::Confirm);
        empty.apply(Action::Save);
        assert_eq!(empty.request, None);
        empty.apply(Action::SaveNew);
        assert_eq!(empty.request, Some(FileRequest::SaveNew), "se puede crear el primero");
    }

    #[test]
    fn shrinking_the_file_list_keeps_the_cursor_in_range() {
        let mut s = files_state();
        s.file_cursor = 2;
        s.set_files(vec!["a.toml".into()]);
        assert_eq!(s.file_cursor, 0);
        s.set_files(Vec::new());
        assert_eq!(s.file_cursor, 0);
    }

    #[test]
    fn success_messages_expire_but_errors_stay() {
        let mut s = UiState::default();
        s.set_status("cargado", false);
        for _ in 0..149 {
            s.tick();
        }
        assert!(s.status.is_some());
        s.tick();
        s.tick();
        assert_eq!(s.status, None);

        s.set_status("fallo", true);
        for _ in 0..10_000 {
            s.tick();
        }
        assert_eq!(s.status, Some(Status { text: "fallo".into(), error: true }));
        s.set_status("ok", false);
        assert_eq!(s.status.as_ref().unwrap().text, "ok");
    }
}
