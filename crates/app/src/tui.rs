//! Interfaz de terminal: pantallas Play y Synth sobre una sesión de tocar.

use crate::audio::AudioArgs;
use crate::midi::Midi;
use crate::session::{self, Mode};
use anyhow::Result;
use clap::Args;
use distrodaw_engine::Event;
use distrodaw_tui::{next_action, Tui, UiState};
use std::time::Duration;

/// Cuadro de la interfaz: unos 30 por segundo.
const FRAME: Duration = Duration::from_millis(33);

#[derive(Args, Debug, Default)]
pub struct TuiArgs {
    /// Fragmento del nombre del puerto MIDI (por defecto, el primer teclado).
    #[arg(long)]
    pub midi: Option<String>,
}

pub fn run(audio_args: &AudioArgs, args: &TuiArgs) -> Result<()> {
    // Sin `--midi` la interfaz arranca aunque no haya teclado conectado.
    let mut session = session::start(audio_args, args.midi.as_deref(), args.midi.is_some(), Mode::Tui)?;
    let rate = session.output.sample_rate;

    let mut state = UiState::default();
    state.midi_port = session.midi_port.clone().unwrap_or_else(|| "(sin teclado MIDI)".into());
    let buffer = session.output.device_buffer.map_or("auto".to_string(), |b| b.to_string());
    state.audio_info = format!("{} · {rate} Hz · buffer {buffer}", session.output.device_name);

    let mut tui = Tui::enter()?;
    while !state.quit {
        while let Ok(msg) = session.midi_rx.pop() {
            if let Midi::NoteOn { note, .. } = msg {
                state.note_on(note);
            }
        }
        state.params = session.shared.params();
        state.voices = session.shared.voices();
        state.set_peak(session.shared.take_peak());
        let report = session.stats.take_report(rate);
        state.cpu_load = report.interval_avg_load as f32;
        state.misses = report.misses;
        state.errors = report.stream_errors;

        tui.draw(&state)?;

        if let Some(action) = next_action(FRAME)? {
            if let Some(edit) = state.apply(action) {
                // Si la cola está llena se pierde este paso; el siguiente lo recupera.
                let _ = session.ui_tx.push(Event::set_param(0, edit.id, edit.value));
            }
        }
    }
    Ok(())
}
