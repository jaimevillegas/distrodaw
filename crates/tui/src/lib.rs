//! Interfaz de terminal de DistroDAW.
//!
//! Pantallas: Play (F1) y Synth (F2); Seq, Song y Files llegan en hitos
//! posteriores. Se dibuja con `ratatui` y se maneja con teclado (y joystick, que
//! produce las mismas [`Action`]).

pub mod format;
pub mod input;
pub mod state;
pub mod theme;
pub mod terminal;
pub mod ui;

pub use terminal::{next_action, Tui};
pub use ui::draw;
pub use theme::{ColorMode, Palette, Rgb, Theme, ThemeError};
pub use state::{Action, Edit, FileRequest, Screen, Status, Step, UiState};
