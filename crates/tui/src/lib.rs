//! Interfaz de terminal de DistroDAW.
//!
//! Pantallas: Play (F1) y Synth (F2); Seq, Song y Files llegan en hitos
//! posteriores. Se dibuja con `ratatui` y se maneja con teclado (y joystick, que
//! produce las mismas [`Action`]).

pub mod format;
pub mod input;
pub mod state;
pub mod terminal;
pub mod ui;

pub use terminal::{next_action, Tui};
pub use ui::draw;
pub use state::{Action, Edit, Screen, Step, UiState};
