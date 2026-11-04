//! Motor de audio de DistroDAW.
//!
//! Regla de oro: nada de lo que se ejecute en el hilo de audio puede
//! reservar memoria, usar locks ni hacer I/O. Este crate no depende de
//! ninguna librería de audio ni de UI para poder probarse y portarse fácil.

pub mod bank;
pub mod env;
pub mod event;
pub mod osc;
pub mod params;
pub mod patch;
pub mod synth;
pub mod voice;

pub use bank::SineBank;
pub use env::{Adsr, EnvCoefs, Stage};
pub use event::{Event, EventKind};
pub use osc::Sine;
pub use synth::{Synth, VOICES};
pub use voice::{midi_to_hz, Voice, Waveform};
pub use patch::{Patch, PatchError};
pub use params::{ParamId, ParamKind, ParamSpec, Params, PARAM_COUNT, SPECS};
