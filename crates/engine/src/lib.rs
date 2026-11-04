//! Motor de audio de DistroDAW.
//!
//! Regla de oro: nada de lo que se ejecute en el hilo de audio puede
//! reservar memoria, usar locks ni hacer I/O. Este crate no depende de
//! ninguna librería de audio ni de UI para poder probarse y portarse fácil.

pub mod osc;

pub use osc::Sine;
