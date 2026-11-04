//! Motor de audio de DistroDAW.
//!
//! Regla de oro: nada de lo que se ejecute en el hilo de audio puede
//! reservar memoria, usar locks ni hacer I/O. Este crate no depende de
//! ninguna librería de audio ni de UI para poder probarse y portarse fácil.

pub mod bank;
pub mod event;
pub mod osc;
pub mod params;

pub use bank::SineBank;
pub use event::{Event, EventKind};
pub use osc::Sine;
pub use params::{ParamId, ParamSpec, Params, PARAM_COUNT, SPECS};
