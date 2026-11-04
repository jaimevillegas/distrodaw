//! Una sesión de tocar: audio + sinte + entrada MIDI + colas hacia el motor.
//! La comparten el modo de texto (`play`) y la interfaz (`tui`).
//!
//! Si el dispositivo de audio desaparece (p. ej. se desconecta el USB), el motor
//! se recupera del stream muerto y se reabre el audio solo cuando el
//! dispositivo vuelve, sin perder parámetros ni colas.

use crate::audio::{self, AudioArgs, AudioInfo, Output};
use crate::midi::{self, Midi};
use crate::shared::Shared;
use crate::stats::AudioStats;
use anyhow::{anyhow, Result};
use distrodaw_engine::{Event, Synth};
use midir::{MidiInput, MidiInputConnection};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Eventos que el hilo de audio atiende por trozo y por cola. El resto espera al siguiente.
const BATCH: usize = 32;
const QUEUE: usize = 256;
/// Cada cuánto se intenta reabrir el audio tras perder el dispositivo.
const RETRY_EVERY: Duration = Duration::from_secs(1);
/// Cuánto se espera a que el stream muerto devuelva el motor.
const ENGINE_WAIT: Duration = Duration::from_secs(2);
/// Frecuencia con la que se crea el motor antes de abrir el dispositivo; se ajusta a la real.
const PLACEHOLDER_RATE: f32 = 48_000.0;

/// Cómo se muestra la sesión; decide si se puede escribir en la consola.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Línea de estado en texto; con `monitor` imprime cada mensaje MIDI.
    Text { monitor: bool },
    /// Interfaz a pantalla completa: no se debe escribir nada por la consola.
    Tui,
}

/// Lo que cambió en el audio desde la última comprobación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioEvent {
    /// El dispositivo desapareció; se reintentará solo.
    Lost,
    /// El dispositivo volvió y el audio suena de nuevo.
    Restored,
}

/// Todo lo que vive en el hilo de audio: el sinte y los consumidores de las colas.
struct Engine {
    synth: Synth,
    ui_rx: Consumer<Event>,
    midi_rx: Consumer<Event>,
    events: [Event; BATCH * 2],
    shared: Arc<Shared>,
}

impl Engine {
    fn new(rate: f32, ui_rx: Consumer<Event>, midi_rx: Consumer<Event>, shared: Arc<Shared>) -> Self {
        Self {
            synth: Synth::new(rate),
            ui_rx,
            midi_rx,
            events: [Event::note_off(0, 0); BATCH * 2],
            shared,
        }
    }

    /// Ajusta el sinte a la frecuencia del dispositivo, conservando los parámetros.
    fn retune(&mut self, rate: f32) {
        if (self.synth.sample_rate() - rate).abs() > 0.5 {
            self.synth = Synth::with_params(rate, self.synth.params().clone());
        }
    }

    fn render(&mut self, out: &mut [f32]) {
        // Primero los cambios de la interfaz, luego las notas.
        let mut n = 0;
        for _ in 0..BATCH {
            let Ok(event) = self.ui_rx.pop() else { break };
            self.events[n] = event;
            n += 1;
        }
        for _ in 0..BATCH {
            let Ok(event) = self.midi_rx.pop() else { break };
            self.events[n] = event;
            n += 1;
        }
        self.synth.render(&self.events[..n], out);
        self.shared.publish_peak(out);
        self.shared.publish_params(self.synth.params());
        self.shared.publish_voices(self.synth.active_voices());
    }
}

/// Envuelve el motor dentro del callback de audio y lo devuelve a la sesión
/// cuando el stream se destruye, para poder reutilizarlo en el siguiente.
struct Handoff {
    engine: Option<Engine>,
    back: Sender<Engine>,
}

impl Handoff {
    fn render(&mut self, out: &mut [f32]) {
        if let Some(engine) = self.engine.as_mut() {
            engine.render(out);
        }
    }
}

impl Drop for Handoff {
    fn drop(&mut self) {
        if let Some(engine) = self.engine.take() {
            let _ = self.back.send(engine);
        }
    }
}

pub struct Session {
    pub stats: Arc<AudioStats>,
    pub shared: Arc<Shared>,
    /// Cambios de parámetros que pide la interfaz.
    pub ui_tx: Producer<Event>,
    /// Mensajes MIDI recibidos, para mostrarlos en la interfaz.
    pub midi_rx: Consumer<Midi>,
    /// Puerto MIDI conectado, si hay.
    pub midi_port: Option<String>,
    /// Datos del último dispositivo abierto (siguen ahí aunque se pierda).
    pub info: AudioInfo,
    output: Option<Output>,
    engine: Option<Engine>,
    back_tx: Sender<Engine>,
    back_rx: Receiver<Engine>,
    audio_args: AudioArgs,
    verbose: bool,
    next_retry: Instant,
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

    let (mut midi_events_tx, midi_events_rx) = RingBuffer::<Event>::new(QUEUE);
    let (ui_tx, ui_rx) = RingBuffer::<Event>::new(QUEUE);
    let (mut notes_tx, midi_rx) = RingBuffer::<Midi>::new(QUEUE);
    let stats = Arc::new(AudioStats::default());
    let shared = Arc::new(Shared::default());
    let (back_tx, back_rx) = mpsc::channel();

    let mut session = Session {
        stats,
        shared: Arc::clone(&shared),
        ui_tx,
        midi_rx,
        midi_port: None,
        info: AudioInfo::default(),
        output: None,
        engine: Some(Engine::new(PLACEHOLDER_RATE, ui_rx, midi_events_rx, shared)),
        back_tx,
        back_rx,
        audio_args: audio_args.clone(),
        verbose: mode != Mode::Tui,
        next_retry: Instant::now(),
        _midi: None,
    };
    session.open_stream()?;

    if let Some(port) = port {
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
        session.midi_port = Some(name);
        session._midi = Some(connection);
    }
    Ok(session)
}

impl Session {
    /// `true` mientras no hay stream de audio abierto (dispositivo perdido).
    pub fn audio_lost(&self) -> bool {
        self.output.is_none()
    }

    /// Abre el dispositivo con el motor que haya disponible. Si falla, el motor
    /// se recupera para el siguiente intento.
    fn open_stream(&mut self) -> Result<()> {
        let mut slot = self.engine.take();
        let back = self.back_tx.clone();
        let result = audio::open(&self.audio_args, &self.stats, self.verbose, |rate| {
            let mut engine = slot.take().expect("el motor debe estar libre para abrir el audio");
            engine.retune(rate);
            let mut handoff = Handoff { engine: Some(engine), back };
            move |out: &mut [f32]| handoff.render(out)
        });
        match result {
            Ok(output) => {
                self.stats.reset_gap();
                self.info = output.info.clone();
                self.output = Some(output);
                Ok(())
            }
            Err(e) => {
                // O no llegó a usarse (sigue en `slot`) o volvió al fallar el stream.
                self.engine = slot.or_else(|| self.back_rx.try_recv().ok());
                Err(e)
            }
        }
    }

    /// Debe llamarse con frecuencia (p. ej. en cada cuadro de la interfaz).
    /// Detecta la pérdida del dispositivo y reintenta abrirlo hasta que vuelva.
    pub fn check_audio(&mut self) -> Option<AudioEvent> {
        if self.output.is_some() {
            if !self.stats.take_device_lost() {
                return None;
            }
            // El hilo de audio ya terminó por su cuenta: soltar el stream no se cuelga
            // y devuelve el motor.
            self.output = None;
            self.engine = self.back_rx.recv_timeout(ENGINE_WAIT).ok();
            self.next_retry = Instant::now() + RETRY_EVERY;
            return Some(AudioEvent::Lost);
        }

        if Instant::now() < self.next_retry {
            return None;
        }
        self.next_retry = Instant::now() + RETRY_EVERY;
        if self.engine.is_none() {
            self.engine = self.back_rx.try_recv().ok();
        }
        if self.engine.is_none() || self.open_stream().is_err() {
            return None;
        }
        self.stats.take_device_lost();
        // Las notas cuyo «soltar» se perdió mientras no había audio no deben quedarse colgadas.
        let _ = self.ui_tx.push(Event::all_notes_off(0));
        Some(AudioEvent::Restored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use distrodaw_engine::{ParamId, EventKind};

    fn engine() -> (Engine, Producer<Event>, Producer<Event>) {
        let (ui_tx, ui_rx) = RingBuffer::new(16);
        let (midi_tx, midi_rx) = RingBuffer::new(16);
        (Engine::new(48_000.0, ui_rx, midi_rx, Arc::new(Shared::default())), ui_tx, midi_tx)
    }

    #[test]
    fn engine_applies_ui_events_before_notes_and_publishes_state() {
        let (mut engine, mut ui, mut midi) = engine();
        // El volumen a 0 llega por la cola de la interfaz y debe aplicarse antes de la nota.
        ui.push(Event::set_param(0, ParamId::Volume, 0.0)).unwrap();
        midi.push(Event::note_on(0, 60, 100)).unwrap();
        let mut out = [0.0f32; 256];
        engine.render(&mut out);
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(engine.shared.voices(), 1);
        assert_eq!(engine.shared.params()[ParamId::Volume as usize], 0.0);
    }

    #[test]
    fn handoff_returns_the_engine_with_its_state_when_dropped() {
        let (engine, _ui, mut midi) = engine();
        midi.push(Event::note_on(0, 64, 100)).unwrap();
        let (back_tx, back_rx) = mpsc::channel();
        let mut handoff = Handoff { engine: Some(engine), back: back_tx };
        handoff.render(&mut [0.0f32; 128]);
        drop(handoff);

        let mut recovered = back_rx.try_recv().expect("el motor debe volver");
        assert_eq!(recovered.synth.active_voices(), 1, "la nota sigue en el sinte");
        // Y es reutilizable: puede seguir procesando.
        recovered.render(&mut [0.0f32; 128]);
    }

    #[test]
    fn retune_keeps_parameters_when_the_sample_rate_changes() {
        let (mut engine, mut ui, _midi) = engine();
        ui.push(Event::set_param(0, ParamId::Release, 2.0)).unwrap();
        engine.render(&mut [0.0f32; 64]);

        engine.retune(48_000.0);
        assert_eq!(engine.synth.sample_rate(), 48_000.0);

        engine.retune(44_100.0);
        assert_eq!(engine.synth.sample_rate(), 44_100.0);
        assert_eq!(engine.synth.params().get(ParamId::Release), 2.0);
    }

    #[test]
    fn all_notes_off_can_be_queued_for_after_a_recovery() {
        let (mut engine, mut ui, mut midi) = engine();
        midi.push(Event::note_on(0, 60, 100)).unwrap();
        engine.render(&mut [0.0f32; 64]);
        ui.push(Event::new(0, EventKind::AllNotesOff)).unwrap();
        engine.render(&mut [0.0f32; 64]);
        // Sigue sonando la relajación, pero ya no hay notas sostenidas.
        assert_eq!(engine.synth.active_voices(), 1);
        engine.synth.render(&[], &mut [0.0f32; 48_000]);
        assert_eq!(engine.synth.active_voices(), 0);
    }

    /// Recorre el camino completo de recuperación con un dispositivo real: simula
    /// la pérdida y comprueba que el audio se reabre conservando el estado.
    /// Se ejecuta con `cargo test -p distrodaw -- --ignored`.
    #[test]
    #[ignore = "necesita un dispositivo de audio"]
    fn session_reopens_the_audio_and_keeps_its_state_after_a_device_loss() {
        use std::thread::sleep;

        fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
            for _ in 0..100 {
                if done() {
                    return;
                }
                sleep(Duration::from_millis(50));
            }
            panic!("tiempo agotado esperando: {what}");
        }

        let args = AudioArgs { device: None, buffer: None, rate: None };
        let mut s = start(&args, None, false, Mode::Tui).expect("abrir audio");
        assert!(!s.audio_lost());
        let rate = s.info.sample_rate;

        s.ui_tx.push(Event::set_param(0, ParamId::Release, 2.0)).unwrap();
        let shared = Arc::clone(&s.shared);
        wait_until("parámetro aplicado", || shared.params()[ParamId::Release as usize] == 2.0);

        s.stats.mark_device_lost();
        assert_eq!(s.check_audio(), Some(AudioEvent::Lost));
        assert!(s.audio_lost());
        assert_eq!(s.check_audio(), None, "no reintenta antes de tiempo");

        wait_until("audio recuperado", || s.check_audio() == Some(AudioEvent::Restored));
        assert!(!s.audio_lost());

        // El mismo motor sigue vivo: conserva los parámetros y vuelve a procesar audio.
        let before = s.stats.take_report(rate).callbacks;
        wait_until("callbacks tras recuperar", || s.stats.take_report(rate).callbacks > before);
        assert_eq!(s.shared.params()[ParamId::Release as usize], 2.0);
    }
}
