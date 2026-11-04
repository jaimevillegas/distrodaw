//! Dibujo de las pantallas. Todos los colores salen de una [`Palette`]: con
//! `Palette::basic()` solo se usan los 16 colores básicos (se ve igual en la
//! consola del kernel y por SSH) y con un tema, colores RGB de 24 bits.

use crate::format::{bar, note_name, param_value, waveform_name};
use crate::state::{Realtime, Screen, UiState};
use crate::theme::Palette;
use distrodaw_engine::{ParamId, ParamKind, SPECS};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

const MIN_WIDTH: u16 = 44;
const MIN_HEIGHT: u16 = 14;
const METER_WIDTH: usize = 16;
const BAR_WIDTH: usize = 20;
const VOICES: usize = distrodaw_engine::VOICES;

/// Bloque con borde y el estilo base de la paleta.
fn bordered<'a>(p: &Palette) -> Block<'a> {
    Block::bordered().style(p.base()).border_style(p.border_style())
}

pub fn draw(frame: &mut Frame, state: &UiState, p: &Palette) {
    let area = frame.area();
    // Pinta todo el fondo con el color del tema (no hace nada con la paleta básica).
    frame.render_widget(Block::new().style(p.base()), area);
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = format!("Terminal demasiado pequeña (mínimo {MIN_WIDTH}x{MIN_HEIGHT})");
        frame.render_widget(Paragraph::new(msg).style(p.base()), area);
        return;
    }
    let [body, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area);
    match state.screen {
        Screen::Play => draw_play(frame, body, state, p),
        Screen::Synth => draw_synth(frame, body, state, p),
        Screen::Files => draw_files(frame, body, state, p),
    }
    draw_footer(frame, footer, state, p);
}

fn draw_play(frame: &mut Frame, area: Rect, state: &UiState, p: &Palette) {
    let title = " DISTRODAW · Play ";
    let block = with_status(
        bordered(p).title(Span::styled(title, p.title_style())),
        state,
        area.width,
        title,
        p,
    );
    let notes = if state.recent_notes.is_empty() {
        "-".to_string()
    } else {
        state.recent_notes.iter().map(|&n| note_name(n)).collect::<Vec<_>>().join(" ")
    };
    let active = state.voices.min(VOICES);
    let slots = format!("{}{}", "■".repeat(active), "□".repeat(VOICES - active));
    let lines = vec![
        row("Patch", &state.patch, p),
        row("MIDI", if state.midi_port.is_empty() { "-" } else { &state.midi_port }, p),
        row("Audio", if state.audio_info.is_empty() { "-" } else { &state.audio_info }, p),
        realtime_row(state.realtime, p),
        Line::raw(""),
        Line::from(vec![
            Span::styled(format!("{:<8}", "Voces"), p.label_style()),
            Span::styled(slots, p.value_style()),
            Span::raw(format!("  {active}/{VOICES}")),
        ]),
        row("Notas", &notes, p),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn realtime_row<'a>(rt: Realtime, p: &Palette) -> Line<'a> {
    let (text, style) = match rt {
        Realtime::Active(prio) => (format!("activo (SCHED_FIFO {prio})"), Style::new().fg(p.ok)),
        Realtime::Denied => ("NO: falta permiso (docs/AUDIO.md)".to_string(), Style::new().fg(p.warn)),
        Realtime::Off => ("desactivado".to_string(), p.label_style()),
        Realtime::Pending => ("-".to_string(), p.label_style()),
    };
    Line::from(vec![Span::styled(format!("{:<8}", "T. real"), p.label_style()), Span::styled(text, style)])
}

fn row<'a>(label: &str, value: &str, p: &Palette) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{label:<8}"), p.label_style()),
        Span::raw(value.to_string()),
    ])
}

fn draw_synth(frame: &mut Frame, area: Rect, state: &UiState, p: &Palette) {
    let title = format!(" SYNTH: {} ", state.patch);
    let block = with_status(
        bordered(p).title(Span::styled(title.clone(), p.title_style())),
        state,
        area.width,
        &title,
        p,
    );
    let wave = waveform_name(state.params[ParamId::Waveform as usize]);
    let mut lines = vec![
        Line::from(vec![
            Span::styled("OSC ", p.label_style()),
            Span::styled(wave, p.accent_style()),
            Span::styled(" ─▶ ", p.label_style()),
            Span::styled("AMP adsr", p.accent_style()),
            Span::styled(" ─▶ ", p.label_style()),
            Span::styled("OUT", p.accent_style()),
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
            p.value_style()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Añade el mensaje de estado, si hay, alineado a la derecha del borde superior.
fn with_status<'a>(
    block: Block<'a>,
    state: &UiState,
    width: u16,
    title: &str,
    p: &Palette,
) -> Block<'a> {
    let Some(status) = &state.status else { return block };
    let color = if status.error { p.alert } else { p.ok };
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

fn draw_files(frame: &mut Frame, area: Rect, state: &UiState, p: &Palette) {
    let title = " DISTRODAW · Files ";
    let block = with_status(
        bordered(p).title(Span::styled(title, p.title_style())),
        state,
        area.width,
        title,
        p,
    );
    let mut lines = vec![
        row("Carpeta", if state.patch_dir.is_empty() { "-" } else { &state.patch_dir }, p),
        Line::raw(""),
    ];
    if state.files.is_empty() {
        lines.push(Line::styled("No hay patches (.toml). Pulsa n para guardar el actual.", p.label_style()));
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
                p.value_style()
            };
            lines.push(Line::from(Span::styled(format!("{marker} {loaded} {name}"), style)));
        }
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_footer(frame: &mut Frame, area: Rect, state: &UiState, p: &Palette) {
    let hints = match state.screen {
        Screen::Play => " F1 Play  F2 Synth  F5 Files  q salir ",
        Screen::Synth => " F1 Play  F2 Synth  F5 Files  ↑↓ elegir  ←→ ajustar  ,. fino  q salir ",
        Screen::Files => " F1 Play  F2 Synth  F5 Files  ↑↓ elegir  Enter cargar  s guardar  n nuevo ",
    };
    let block = bordered(p).title_bottom(Span::styled(hints, p.label_style()));

    if state.audio_lost {
        let alert = Line::from(vec![
            Span::styled("AUDIO PERDIDO", Style::new().fg(p.alert).add_modifier(Modifier::BOLD)),
            Span::styled(" · se reintenta solo · las notas no suenan", p.label_style()),
        ]);
        frame.render_widget(Paragraph::new(alert).block(block), area);
        return;
    }
    let cpu_color = if state.cpu_load >= 0.8 || state.misses > 0 { p.alert } else { p.ok };
    let xruns = state.misses + state.errors;
    let mut spans = vec![Span::styled("OUT ", p.label_style())];
    spans.extend(meter(state.peak, p));
    spans.extend([
        Span::styled("  voces ", p.label_style()),
        Span::raw(format!("{}/{VOICES}", state.voices.min(VOICES))),
        Span::styled("  cpu ", p.label_style()),
        Span::styled(format!("{:.0}%", state.cpu_load * 100.0), Style::new().fg(cpu_color)),
        Span::styled("  xruns ", p.label_style()),
        Span::styled(xruns.to_string(), Style::new().fg(if xruns > 0 { p.alert } else { p.ok })),
        Span::styled("  tarde ", p.label_style()),
        Span::styled(state.late.to_string(), Style::new().fg(if state.late > 0 { p.warn } else { p.ok })),
        Span::styled("  hueco ", p.label_style()),
        Span::raw(format!("{:.1}ms", state.max_gap_ms)),
    ]);
    frame.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}

/// Medidor de nivel: cada casilla lleva el color de su posición (verde,
/// ámbar a partir del 70 % y rojo a partir del 90 %), como un medidor de verdad.
fn meter<'a>(level: f32, p: &Palette) -> Vec<Span<'a>> {
    let filled = (level.clamp(0.0, 1.0) * METER_WIDTH as f32).round() as usize;
    (0..METER_WIDTH)
        .map(|i| {
            if i >= filled {
                return Span::styled("░", p.label_style());
            }
            let position = (i + 1) as f32 / METER_WIDTH as f32;
            let color = if position > 0.9 {
                p.alert
            } else if position > 0.7 {
                p.warn
            } else {
                p.ok
            };
            Span::styled("▓", Style::new().fg(color))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Action;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render(state: &UiState, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| draw(f, state, &Palette::basic())).unwrap();
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

    #[test]
    fn footer_warns_loudly_when_the_audio_device_is_lost() {
        let mut s = UiState::default();
        s.audio_lost = true;
        let out = render(&s, 80, 20);
        assert!(contains(&out, "AUDIO PERDIDO"));
        assert!(!contains(&out, "OUT ▓"), "no se enseña un medidor que ya no se mueve");
        s.audio_lost = false;
        assert!(contains(&render(&s, 80, 20), "OUT "));
    }

    use crate::theme::Theme;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;

    fn render_buffer(state: &UiState, palette: &Palette, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| draw(f, state, palette)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn dracula() -> Palette {
        Palette::from_theme(&Theme::default())
    }

    #[test]
    fn a_theme_paints_the_whole_screen_background_in_rgb() {
        let buffer = render_buffer(&UiState::default(), &dracula(), 80, 25);
        let bg = Color::Rgb(0x28, 0x2a, 0x36);
        for (x, y) in [(0, 0), (79, 24), (40, 12), (79, 0), (0, 24)] {
            assert_eq!(buffer[(x, y)].bg, bg, "celda {x},{y}");
        }
        // Una celda con texto usa el color de texto del tema.
        let patch_row = (0..25).find(|&y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>().contains("Patch")).unwrap();
        let value = (0..80).map(|x| buffer[(x, patch_row)].symbol().to_string()).collect::<String>();
        let x_init = value.find("Init").unwrap() as u16;
        assert_eq!(buffer[(x_init, patch_row)].fg, Color::Rgb(0xf8, 0xf8, 0xf2));
    }

    #[test]
    fn borders_and_titles_take_their_theme_colors() {
        let buffer = render_buffer(&UiState::default(), &dracula(), 80, 25);
        assert_eq!(buffer[(0, 0)].fg, Color::Rgb(0x62, 0x72, 0xa4), "borde = label");
        let title_x = (0..80).find(|&x| buffer[(x, 0)].symbol() == "D").unwrap();
        assert_eq!(buffer[(title_x, 0)].fg, Color::Rgb(0xbd, 0x93, 0xf9), "título");
    }

    #[test]
    fn the_basic_palette_never_sets_a_background_or_rgb_colors() {
        let buffer = render_buffer(&UiState::default(), &Palette::basic(), 80, 25);
        for y in 0..25 {
            for x in 0..80 {
                let cell = &buffer[(x, y)];
                assert_eq!(cell.bg, Color::Reset, "celda {x},{y}");
                assert!(!matches!(cell.fg, Color::Rgb(..)), "celda {x},{y}");
            }
        }
    }

    #[test]
    fn the_level_meter_changes_color_along_its_length() {
        let mut s = UiState::default();
        s.peak = 1.0;
        let p = dracula();
        let buffer = render_buffer(&s, &p, 80, 25);
        let row = 25 - 2;
        let cells: Vec<Color> = (0..80)
            .filter(|&x| buffer[(x, row)].symbol() == "▓")
            .map(|x| buffer[(x, row)].fg)
            .collect();
        assert_eq!(cells.len(), METER_WIDTH);
        assert_eq!(cells[0], p.ok);
        assert_eq!(cells[METER_WIDTH - 1], p.alert);
        assert!(cells.contains(&p.warn), "debe haber tramo ámbar");
    }

    #[test]
    fn a_quiet_meter_stays_green_and_the_empty_part_is_dim() {
        let mut s = UiState::default();
        s.peak = 0.25;
        let p = dracula();
        let buffer = render_buffer(&s, &p, 80, 25);
        let row = 25 - 2;
        let filled: Vec<Color> = (0..80).filter(|&x| buffer[(x, row)].symbol() == "▓").map(|x| buffer[(x, row)].fg).collect();
        assert!(filled.iter().all(|&c| c == p.ok));
        let empty = (0..80).find(|&x| buffer[(x, row)].symbol() == "░").unwrap();
        assert_eq!(buffer[(empty, row)].fg, p.label);
    }

    #[test]
    fn footer_shows_late_callbacks_and_the_largest_gap() {
        let mut s = UiState::default();
        s.late = 3;
        s.max_gap_ms = 12.34;
        let out = render(&s, 80, 20);
        assert!(contains(&out, "tarde 3"));
        assert!(contains(&out, "hueco 12.3ms"));
        // Con valores grandes sigue cabiendo en 80 columnas.
        s.late = 12_345;
        s.misses = 999;
        s.max_gap_ms = 123.4;
        let out = render(&s, 80, 20);
        let footer = &out[out.len() - 2];
        assert!(footer.contains("hueco 123.4ms"), "{footer}");
    }

    #[test]
    fn play_screen_reports_the_real_time_status() {
        let mut s = UiState::default();
        s.realtime = Realtime::Active(70);
        assert!(contains(&render(&s, 80, 20), "activo (SCHED_FIFO 70)"));
        s.realtime = Realtime::Denied;
        assert!(contains(&render(&s, 80, 20), "falta permiso"));
        s.realtime = Realtime::Off;
        assert!(contains(&render(&s, 80, 20), "desactivado"));
    }
}
