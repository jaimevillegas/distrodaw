//! Una sesión de tocar: audio + sinte + entrada MIDI + colas hacia el motor.
//! La comparten el modo de texto (`play`) y la interfaz (`tui`).

use crate::audio::{self, AudioArgs, Output};
use crate::midi::{self, Midi};
use crate::shared::Shared;
use crate::stats::AudioStats;
use anyhow::{anyhow, Result};
use distrodaw_engine::{Event, Synth};
use midir::{MidiInput, MidiInputConnection};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;

/// Eventos que el hilo de audio atiende por trozo y por cola. El resto espera al siguiente.
const BATCH: usize = 32;
const QUEUE: usize = 256;

/// Cómo se muestra la sesión; decide si se puede escribir en la consola.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Línea de estado en texto; con `monitor` imprime cada mensaje MIDI.
    Text { monitor: bool },
    /// Interfaz a pantalla completa: no se debe escribir nada por la consola.
    Tui,
}

pub struct Session {
    pub output: Output,
    pub stats: Arc<AudioStats>,
    pub shared: Arc<Shared>,
    /// Cambios de parámetros que pide la interfaz.
    pub ui_tx: Producer<Event>,
    /// Mensajes MIDI recibidos, para mostrarlos en la interfaz.
    pub midi_rx: Consumer<Midi>,
    /// Puerto MIDI conectado, si hay.
    pub midi_port: Option<String>,
    _midi: Option<MidiInputConnection<()>>,
}

/// Arranca audio y MIDI. Con `midi_required` falla si no hay teclado; si no,
/// sigue sin él (la interfaz se puede usar y editar igualmente).
pub fn start(
    audio_args: &AudioArgs,
    midi_filter: Option<&str>,
    midi_required: bool,
    mode: Mode,
) -> Result<Session> {
    let monitor = mode == Mode::Text { monitor: true };
    let input = MidiInput::new("distrodaw")?;
    let port = match midi::pick_port(&input, midi_filter) {
        Ok(port) => Some(port),
        Err(e) if midi_required => return Err(e),
        Err(_) => None,
    };

    let (mut midi_events_tx, mut midi_events_rx) = RingBuffer::<Event>::new(QUEUE);
    let (ui_tx, mut ui_rx) = RingBuffer::<Event>::new(QUEUE);
    let (mut notes_tx, midi_rx) = RingBuffer::<Midi>::new(QUEUE);
    let stats = Arc::new(AudioStats::default());
    let shared = Arc::new(Shared::default());

    let audio_shared = Arc::clone(&shared);
    let output = audio::open(audio_args, &stats, mode != Mode::Tui, move |rate| {
        let mut synth = Synth::new(rate);
        let mut events = [Event::note_off(0, 0); BATCH * 2];
        move |out: &mut [f32]| {
            // Primero los cambios de la interfaz, luego las notas.
            let mut n = 0;
            for _ in 0..BATCH {
                let Ok(event) = ui_rx.pop() else { break };
                events[n] = event;
                n += 1;
            }
            for _ in 0..BATCH {
                let Ok(event) = midi_events_rx.pop() else { break };
                events[n] = event;
                n += 1;
            }
            synth.render(&events[..n], out);
            audio_shared.publish_peak(out);
            audio_shared.publish_params(synth.params());
            audio_shared.publish_voices(synth.active_voices());
        }
    })?;

    let (midi_port, connection) = match port {
        None => (None, None),
        Some(port) => {
            let name = input.port_name(&port).map_err(|e| anyhow!("{e}"))?;
            let connection = input
                .connect(
                    &port,
                    "distrodaw-in",
                    move |_timestamp, bytes, _| {
                        let Some(msg) = midi::parse(bytes) else { return };
                        if monitor {
                            println!("{msg:?}");
                        }
                        // Los eventos en vivo caen en el frame 0 del siguiente trozo de
                        // audio (< 1 ms con buffers de 64 frames). Si una cola se llena, se descarta.
                        if let Some(kind) = midi::to_event(msg) {
                            let _ = midi_events_tx.push(Event::new(0, kind));
                        }
                        let _ = notes_tx.push(msg);
                    },
                    (),
                )
                .map_err(|e| anyhow!("no se pudo abrir el puerto MIDI: {e}"))?;
            (Some(name), Some(connection))
        }
    };

    Ok(Session { output, stats, shared, ui_tx, midi_rx, midi_port, _midi: connection })
}
