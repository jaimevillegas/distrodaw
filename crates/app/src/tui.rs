//! Interfaz de terminal: pantallas Play, Synth y Files sobre una sesión de tocar.

use crate::audio::AudioArgs;
use crate::midi::Midi;
use crate::patches::{self, Watcher};
use crate::themes;
use crate::stats;
use crate::session::{self, AudioEvent, Mode, Session};
use anyhow::Result;
use clap::{Args, ValueEnum};
use distrodaw_engine::Patch;
use distrodaw_tui::{next_action, ColorMode, FileRequest, Palette, Realtime, Tui, UiState};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Cuadro de la interfaz: unos 30 por segundo.
const FRAME: Duration = Duration::from_millis(33);
/// Cada cuánto se vuelve a leer la lista de patches de la carpeta.
const REFRESH_FILES: Duration = Duration::from_secs(1);
/// Nombre del patch sin nombre propio; al guardarlo se usa el del archivo.
const UNNAMED: &str = "Init";

/// Cuántos colores usar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorChoice {
    /// 24 bits si `COLORTERM` lo indica, si no 16 colores.
    Auto,
    /// Solo los 16 colores básicos (consola del kernel).
    Basic,
    /// Fuerza 24 bits aunque el terminal no lo anuncie.
    Truecolor,
}

#[derive(Args, Debug)]
pub struct TuiArgs {
    /// Fragmento del nombre del puerto MIDI (por defecto, el primer teclado).
    #[arg(long)]
    pub midi: Option<String>,

    /// Patch (.toml) que se carga al arrancar y se recarga solo al guardarlo.
    #[arg(long)]
    pub patch: Option<PathBuf>,

    /// Carpeta de patches para la pantalla Files.
    #[arg(long, default_value = "patches")]
    pub patch_dir: PathBuf,

    /// Tema de color (nombre en `--theme-dir` o ruta a un .toml). Necesita 24 bits.
    #[arg(long)]
    pub theme: Option<String>,

    /// Carpeta de temas.
    #[arg(long, default_value = "themes")]
    pub theme_dir: PathBuf,

    /// Cuántos colores usar.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
}

impl Default for TuiArgs {
    fn default() -> Self {
        Self {
            midi: None,
            patch: None,
            patch_dir: PathBuf::from("patches"),
            theme: None,
            theme_dir: PathBuf::from("themes"),
            color: ColorChoice::Auto,
        }
    }
}

pub fn run(audio_args: &AudioArgs, args: &TuiArgs) -> Result<()> {
    // Sin `--midi` la interfaz arranca aunque no haya teclado conectado.
    let mut session = session::start(audio_args, args.midi.as_deref(), args.midi.is_some(), Mode::Tui)?;
    let mut state = UiState::default();
    state.midi_port = session.midi_port.clone().unwrap_or_else(|| "(sin teclado MIDI)".into());
    let buffer = session.info.device_buffer.map_or("auto".to_string(), |b| b.to_string());
    state.audio_info = format!("{} · {} Hz · buffer {buffer}", session.info.device_name, session.info.sample_rate);
    state.patch_dir = args.patch_dir.display().to_string();
    state.set_files(patches::file_names(&args.patch_dir));

    // Un patch que no carga no impide arrancar: se avisa y se sigue vigilando el
    // archivo, de modo que al corregirlo se recarga solo.
    let mut watcher = args.patch.clone().map(Watcher::new);
    if let Some(path) = &args.patch {
        match patches::load(path) {
            Ok(patch) => show_loaded(&mut state, &mut session, path, &patch, "Cargado"),
            Err(e) => state.set_status(e, true),
        }
    }

    let (palette, note) = choose_palette(args, std::env::var("COLORTERM").ok().as_deref());
    if let Some((text, error)) = note {
        state.set_status(text, error);
    }

    let mut tui = Tui::enter(palette)?;
    let mut last_refresh = Instant::now();
    while !state.quit {
        while let Ok(msg) = session.midi_rx.pop() {
            if let Midi::NoteOn { note, .. } = msg {
                state.note_on(note);
            }
        }
        state.params = session.shared.params();
        state.voices = session.shared.voices();
        state.set_peak(session.shared.take_peak());
        match session.check_audio() {
            Some(AudioEvent::Lost) => {
                state.audio_lost = true;
                state.set_status("Audio perdido: el dispositivo se desconectó. Se reintenta solo", true);
            }
            Some(AudioEvent::Restored) => {
                state.audio_lost = false;
                state.set_status("Audio recuperado", false);
            }
            None => {}
        }
        state.audio_lost = session.audio_lost();
        let report = session.stats.take_report(session.info.sample_rate);
        state.cpu_load = report.interval_avg_load as f32;
        state.misses = report.misses;
        state.errors = report.stream_errors + report.xruns;
        state.late = report.late;
        state.max_gap_ms = report.max_gap_ms as f32;
        state.realtime = match report.realtime {
            stats::Realtime::Pending => Realtime::Pending,
            stats::Realtime::Off => Realtime::Off,
            stats::Realtime::Active(p) => Realtime::Active(p),
            stats::Realtime::Denied(_) => Realtime::Denied,
        };
        state.tick();

        if last_refresh.elapsed() >= REFRESH_FILES {
            last_refresh = Instant::now();
            state.set_files(patches::file_names(&args.patch_dir));
        }

        if let Some(result) = watcher.as_mut().and_then(Watcher::poll) {
            let path = watcher.as_ref().map(|w| w.path().to_path_buf()).unwrap_or_default();
            match result {
                Ok(patch) => show_loaded(&mut state, &mut session, &path, &patch, "Recargado"),
                // El sonido anterior se mantiene; el mensaje va en rojo y empieza por el archivo.
                Err(e) => state.set_status(e, true),
            }
        }

        if let Some(request) = state.request.take() {
            handle_file_request(request, &args.patch_dir, &mut state, &mut session, &mut watcher);
        }

        tui.draw(&state)?;

        if let Some(action) = next_action(FRAME)? {
            if let Some(edit) = state.apply(action) {
                // Si la cola está llena se pierde este paso; el siguiente lo recupera.
                let _ = session.ui_tx.push(distrodaw_engine::Event::set_param(0, edit.id, edit.value));
            }
        }
    }
    Ok(())
}

/// Elige la paleta según los colores del terminal y el tema pedido. Devuelve
/// además un aviso para mostrar si no se pudo aplicar lo pedido.
fn choose_palette(args: &TuiArgs, colorterm: Option<&str>) -> (Palette, Option<(String, bool)>) {
    let mode = match args.color {
        ColorChoice::Auto => ColorMode::detect(colorterm),
        ColorChoice::Basic => ColorMode::Basic,
        ColorChoice::Truecolor => ColorMode::TrueColor,
    };
    let Some(name) = &args.theme else { return (Palette::basic(), None) };
    if mode == ColorMode::Basic {
        let note = format!("Tema {name}: el terminal no anuncia 24 bits, se usan 16 colores (--color truecolor lo fuerza)");
        return (Palette::basic(), Some((note, false)));
    }
    match themes::load(name, &args.theme_dir) {
        Ok(theme) => (Palette::from_theme(&theme), None),
        Err(e) => (Palette::basic(), Some((e, true))),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Aplica un patch al motor y lo refleja en la interfaz.
fn show_loaded(state: &mut UiState, session: &mut Session, path: &Path, patch: &Patch, verb: &str) {
    patches::send(patch, &mut session.ui_tx);
    state.patch = patch.name.clone();
    state.loaded_file = Some(file_name(path));
    state.set_status(format!("{verb} {}", file_name(path)), false);
}

fn handle_file_request(
    request: FileRequest,
    dir: &Path,
    state: &mut UiState,
    session: &mut Session,
    watcher: &mut Option<Watcher>,
) {
    let path = match request {
        FileRequest::Load(i) | FileRequest::Save(i) => match state.files.get(i) {
            Some(name) => dir.join(name),
            None => return,
        },
        FileRequest::SaveNew => patches::new_file_path(dir),
    };

    match request {
        FileRequest::Load(_) => match patches::load(&path) {
            Ok(patch) => {
                show_loaded(state, session, &path, &patch, "Cargado");
                *watcher = Some(Watcher::new(path));
            }
            Err(e) => state.set_status(e, true),
        },
        FileRequest::Save(_) | FileRequest::SaveNew => {
            let name = if state.patch == UNNAMED {
                path.file_stem().map_or_else(|| UNNAMED.to_string(), |s| s.to_string_lossy().into_owned())
            } else {
                state.patch.clone()
            };
            let mut patch = Patch { name, ..Patch::default() };
            for (id, value) in distrodaw_engine::ParamId::ALL.into_iter().zip(state.params) {
                patch.params.set(id, value);
            }
            let saved = fs::create_dir_all(dir).and_then(|()| patches::save(&path, &patch));
            match saved {
                Ok(()) => {
                    state.patch = patch.name;
                    state.loaded_file = Some(file_name(&path));
                    state.set_status(format!("Guardado {}", file_name(&path)), false);
                    state.set_files(patches::file_names(dir));
                    // El archivo recién escrito ya es lo que suena: no hace falta recargarlo.
                    let mut new_watcher = Watcher::new(path);
                    new_watcher.mark_current();
                    *watcher = Some(new_watcher);
                }
                Err(e) => state.set_status(format!("No se pudo guardar {}: {e}", file_name(&path)), true),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(theme: Option<&str>, color: ColorChoice) -> TuiArgs {
        let themes = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
        TuiArgs { theme: theme.map(String::from), theme_dir: themes, color, ..TuiArgs::default() }
    }

    #[test]
    fn without_a_theme_the_basic_palette_is_used() {
        let (p, note) = choose_palette(&args(None, ColorChoice::Auto), Some("truecolor"));
        assert_eq!(p, Palette::basic());
        assert!(note.is_none());
    }

    #[test]
    fn a_theme_applies_when_the_terminal_has_24_bits() {
        let (p, note) = choose_palette(&args(Some("nord"), ColorChoice::Auto), Some("truecolor"));
        assert!(note.is_none());
        assert!(p.background.is_some());
        let forced = choose_palette(&args(Some("nord"), ColorChoice::Truecolor), None).0;
        assert_eq!(p, forced, "--color truecolor equivale a que el terminal lo anuncie");
    }

    #[test]
    fn a_theme_on_a_16_color_terminal_falls_back_and_says_so() {
        let (p, note) = choose_palette(&args(Some("nord"), ColorChoice::Auto), None);
        assert_eq!(p, Palette::basic());
        assert!(note.unwrap().0.contains("16 colores"));
        let (p, _) = choose_palette(&args(Some("nord"), ColorChoice::Basic), Some("truecolor"));
        assert_eq!(p, Palette::basic(), "--color basic manda sobre COLORTERM");
    }

    #[test]
    fn a_broken_theme_falls_back_with_an_error() {
        let (p, note) = choose_palette(&args(Some("no-existe"), ColorChoice::Truecolor), None);
        assert_eq!(p, Palette::basic());
        let (text, error) = note.unwrap();
        assert!(error && text.contains("no-existe"), "{text}");
    }
}
