//! Dibujo de las pantallas. Solo se usan los 16 colores básicos para que se
//! vea igual en la consola del kernel y por SSH.

use crate::format::{bar, note_name, param_value, waveform_name};
use crate::state::{Screen, UiState};
use distrodaw_engine::{ParamId, ParamKind, SPECS};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

const MIN_WIDTH: u16 = 44;
const MIN_HEIGHT: u16 = 14;
const METER_WIDTH: usize = 16;
const BAR_WIDTH: usize = 20;
const VOICES: usize = distrodaw_engine::VOICES;

fn title_style() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

fn label_style() -> Style {
    Style::new().fg(Color::DarkGray)
}

pub fn draw(frame: &mut Frame, state: &UiState) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = format!("Terminal demasiado pequeña (mínimo {MIN_WIDTH}x{MIN_HEIGHT})");
        frame.render_widget(Paragraph::new(msg), area);
        return;
    }
    let [body, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area);
    match state.screen {
        Screen::Play => draw_play(frame, body, state),
        Screen::Synth => draw_synth(frame, body, state),
        Screen::Files => draw_files(frame, body, state),
    }
    draw_footer(frame, footer, state);
}

fn draw_play(frame: &mut Frame, area: Rect, state: &UiState) {
    let title = " DISTRODAW · Play ";
    let block = with_status(
        Block::bordered().title(Span::styled(title, title_style())),
        state,
        area.width,
        title,
    );
    let notes = if state.recent_notes.is_empty() {
        "-".to_string()
    } else {
        state.recent_notes.iter().map(|&n| note_name(n)).collect::<Vec<_>>().join(" ")
    };
    let active = state.voices.min(VOICES);
    let slots = format!("{}{}", "■".repeat(active), "□".repeat(VOICES - active));
    let lines = vec![
        row("Patch", &state.patch),
        row("MIDI", if state.midi_port.is_empty() { "-" } else { &state.midi_port }),
        row("Audio", if state.audio_info.is_empty() { "-" } else { &state.audio_info }),
        Line::raw(""),
        Line::from(vec![
            Span::styled(format!("{:<8}", "Voces"), label_style()),
            Span::styled(slots, Style::new().fg(Color::Green)),
            Span::raw(format!("  {active}/{VOICES}")),
        ]),
        row("Notas", &notes),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn row<'a>(label: &str, value: &str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{label:<8}"), label_style()),
        Span::raw(value.to_string()),
    ])
}

fn draw_synth(frame: &mut Frame, area: Rect, state: &UiState) {
    let title = format!(" SYNTH: {} ", state.patch);
    let block = with_status(
        Block::bordered().title(Span::styled(title.clone(), title_style())),
        state,
        area.width,
        &title,
    );
    let wave = waveform_name(state.params[ParamId::Waveform as usize]);
    let mut lines = vec![
        Line::from(vec![
            Span::styled("OSC ", label_style()),
            Span::styled(wave, Style::new().fg(Color::Yellow)),
            Span::styled(" ─▶ ", label_style()),
            Span::styled("AMP adsr", Style::new().fg(Color::Yellow)),
            Span::styled(" ─▶ ", label_style()),
            Span::styled("OUT", Style::new().fg(Color::Yellow)),
        ]),
        Line::raw(""),
    ];
    for (i, spec) in SPECS.iter().enumerate() {
        let value = state.params[i];
        let selected = i == state.selected;
        let marker = if selected { "▶" } else { " " };
        let fraction = if spec.kind == ParamKind::Stepped {
            (value - spec.min) / (spec.max - spec.min)
        } else {
            spec.to_normalized(value)
        };
        let text = format!(
            "{marker} {:<9} {} {}",
            spec.name,
            bar(fraction, BAR_WIDTH),
            param_value(spec.id, value)
        );
        let style = if selected {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new().fg(Color::Green)
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Añade el mensaje de estado, si hay, alineado a la derecha del borde superior.
fn with_status<'a>(block: Block<'a>, state: &UiState, width: u16, title: &str) -> Block<'a> {
    let Some(status) = &state.status else { return block };
    let color = if status.error { Color::Red } else { Color::Green };
    // Deja sitio al título de la izquierda, a los bordes y a los espacios del mensaje.
    let max = usize::from(width).saturating_sub(title.chars().count() + 6).max(10);
    let text = clip(&status.text, max);
    block.title(Line::from(Span::styled(format!(" {text} "), Style::new().fg(color))).right_aligned())
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

fn draw_files(frame: &mut Frame, area: Rect, state: &UiState) {
    let title = " DISTRODAW · Files ";
    let block = with_status(
        Block::bordered().title(Span::styled(title, title_style())),
        state,
        area.width,
        title,
    );
    let mut lines = vec![
        row("Carpeta", if state.patch_dir.is_empty() { "-" } else { &state.patch_dir }),
        Line::raw(""),
    ];
    if state.files.is_empty() {
        lines.push(Line::styled("No hay patches (.toml). Pulsa n para guardar el actual.", label_style()));
    } else {
        // Ventana de filas que mantiene visible el cursor.
        let visible = usize::from(area.height).saturating_sub(2 + lines.len()).max(1);
        let first = state.file_cursor.saturating_sub(visible - 1);
        for (i, name) in state.files.iter().enumerate().skip(first).take(visible) {
            let marker = if i == state.file_cursor { "▶" } else { " " };
            let loaded = if state.loaded_file.as_deref() == Some(name.as_str()) { "●" } else { " " };
            let style = if i == state.file_cursor {
                Style::new().add_modifier(Modifier::REVERSED)
            } else {
                Style::new().fg(Color::Green)
            };
            lines.push(Line::from(Span::styled(format!("{marker} {loaded} {name}"), style)));
        }
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_footer(frame: &mut Frame, area: Rect, state: &UiState) {
    let hints = match state.screen {
        Screen::Play => " F1 Play  F2 Synth  F5 Files  q salir ",
        Screen::Synth => " F1 Play  F2 Synth  F5 Files  ↑↓ elegir  ←→ ajustar  ,. fino  q salir ",
        Screen::Files => " F1 Play  F2 Synth  F5 Files  ↑↓ elegir  Enter cargar  s guardar  n nuevo ",
    };
    let block = Block::bordered().title_bottom(Span::styled(hints, label_style()));

    let level_color = if state.peak >= 0.9 {
        Color::Red
    } else if state.peak >= 0.7 {
        Color::Yellow
    } else {
        Color::Green
    };
    let cpu_color = if state.cpu_load >= 0.8 || state.misses > 0 { Color::Red } else { Color::Green };
    let line = Line::from(vec![
        Span::styled("OUT ", label_style()),
        Span::styled(bar(state.peak, METER_WIDTH), Style::new().fg(level_color)),
        Span::styled("  voces ", label_style()),
        Span::raw(format!("{}/{VOICES}", state.voices.min(VOICES))),
        Span::styled("  cpu ", label_style()),
        Span::styled(format!("{:.0}%", state.cpu_load * 100.0), Style::new().fg(cpu_color)),
        Span::styled("  xruns ", label_style()),
        Span::styled(
            (state.misses + state.errors).to_string(),
            Style::new().fg(if state.misses + state.errors > 0 { Color::Red } else { Color::Green }),
        ),
    ]);
    frame.render_widget(Paragraph::new(line).block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Action;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render(state: &UiState, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| draw(f, state)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| (0..width).map(|x| buffer[(x, y)].symbol().to_string()).collect::<String>())
            .collect()
    }

    fn contains(lines: &[String], needle: &str) -> bool {
        lines.iter().any(|l| l.contains(needle))
    }

    #[test]
    fn play_screen_shows_status_voices_and_notes() {
        let mut s = UiState::default();
        s.voices = 3;
        s.midi_port = "Keystation".into();
        s.note_on(60);
        s.note_on(64);
        let out = render(&s, 80, 20);
        assert!(contains(&out, "Play"));
        assert!(contains(&out, "Init"));
        assert!(contains(&out, "Keystation"));
        assert!(contains(&out, "3/8"));
        assert!(contains(&out, "C4 E4"));
        assert!(contains(&out, "■■■□□□□□"));
    }

    #[test]
    fn synth_screen_lists_every_parameter_and_marks_the_selection() {
        let mut s = UiState::default();
        s.apply(Action::Screen(Screen::Synth));
        s.apply(Action::Down);
        let out = render(&s, 80, 20);
        assert!(contains(&out, "SYNTH: Init"));
        for spec in SPECS {
            assert!(contains(&out, spec.name), "falta {}", spec.name);
        }
        let marked: Vec<&String> = out.iter().filter(|l| l.contains('▶') && l.contains("attack")).collect();
        assert_eq!(marked.len(), 1, "la fila seleccionada debe llevar ▶");
        assert!(contains(&out, "10 ms"));
        assert!(contains(&out, "OSC saw"));
    }

    #[test]
    fn footer_shows_meter_cpu_and_hints() {
        let mut s = UiState::default();
        s.peak = 0.5;
        s.cpu_load = 0.123;
        s.misses = 2;
        let out = render(&s, 80, 20);
        assert!(contains(&out, "OUT ▓▓▓▓▓▓▓▓░░░░░░░░"));
        assert!(contains(&out, "cpu 12%"));
        assert!(contains(&out, "xruns 2"));
        assert!(contains(&out, "q salir"));
    }

    #[test]
    fn tiny_terminals_get_a_message_instead_of_a_broken_layout() {
        let out = render(&UiState::default(), 30, 8);
        assert!(contains(&out, "demasiado pequeña"));
    }

    #[test]
    fn fits_a_standard_80x25_console() {
        let mut s = UiState::default();
        s.apply(Action::Screen(Screen::Synth));
        let out = render(&s, 80, 25);
        assert!(out.iter().all(|l| l.chars().count() == 80));
    }

    fn files_state() -> UiState {
        let mut s = UiState::default();
        s.patch_dir = "patches".into();
        s.set_files(vec!["bass.toml".into(), "lead.toml".into(), "pad.toml".into()]);
        s.loaded_file = Some("lead.toml".into());
        s.apply(Action::Screen(Screen::Files));
        s
    }

    #[test]
    fn files_screen_lists_patches_marks_cursor_and_loaded_one() {
        let mut s = files_state();
        s.apply(Action::Down);
        let out = render(&s, 80, 20);
        assert!(contains(&out, "Files"));
        assert!(contains(&out, "patches"));
        assert!(contains(&out, "bass.toml"));
        assert!(out.iter().any(|l| l.contains("▶ ● lead.toml")), "cursor y cargado: {out:#?}");
        assert!(out.iter().any(|l| l.contains("  pad.toml")));
        assert!(contains(&out, "Enter cargar"));
    }

    #[test]
    fn files_screen_explains_an_empty_folder() {
        let mut s = UiState::default();
        s.apply(Action::Screen(Screen::Files));
        assert!(contains(&render(&s, 80, 20), "No hay patches"));
    }

    #[test]
    fn files_list_scrolls_to_keep_the_cursor_visible() {
        let mut s = UiState::default();
        s.apply(Action::Screen(Screen::Files));
        s.set_files((0..40).map(|i| format!("p{i:02}.toml")).collect());
        s.file_cursor = 39;
        let out = render(&s, 80, 16);
        assert!(contains(&out, "▶   p39.toml"));
        assert!(!contains(&out, "p00.toml"));
    }

    #[test]
    fn a_typical_reload_error_fits_whole_in_an_80_column_terminal() {
        let mut s = UiState::default();
        s.set_status("live.toml: envelope.sustain = 9 fuera de rango (0 a 1)", true);
        let out = render(&s, 80, 20);
        assert!(contains(&out, "envelope.sustain = 9 fuera de rango (0 a 1)"), "{out:#?}");
    }

    #[test]
    fn status_shows_on_every_screen_and_long_text_is_clipped() {
        let mut s = UiState::default();
        s.set_status("Cargado pad.toml", false);
        for screen in Screen::ALL {
            s.apply(Action::Screen(screen));
            assert!(contains(&render(&s, 80, 20), "Cargado pad.toml"), "{screen:?}");
        }
        s.set_status("x".repeat(200), true);
        let out = render(&s, 80, 20);
        assert!(contains(&out, "…"));
        assert!(out.iter().all(|l| l.chars().count() == 80));
    }
}
