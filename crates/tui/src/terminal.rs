//! Entrada y salida del modo de terminal de la interfaz.

use crate::input::action_for_key;
use crate::state::{Action, UiState};
use crate::theme::Palette;
use ratatui::crossterm::event::{self, Event};
use ratatui::DefaultTerminal;
use std::io;
use std::time::Duration;

/// Terminal en modo raw y pantalla alternativa. Al soltarlo (también si hay un
/// pánico) deja la terminal como estaba.
pub struct Tui {
    terminal: DefaultTerminal,
    palette: Palette,
    /// Estado de la última pantalla pintada; `None` obliga a pintar de nuevo.
    last: Option<UiState>,
}

impl Tui {
    pub fn enter(palette: Palette) -> io::Result<Self> {
        Ok(Self { terminal: ratatui::try_init()?, palette, last: None })
    }

    /// Pinta la pantalla, salvo que no haya cambiado nada visible desde la última vez.
    /// Dibujar cuesta tiempo de CPU proporcional al tamaño del terminal, que en una
    /// Pi 2 compite con el audio.
    pub fn draw(&mut self, state: &UiState) -> io::Result<()> {
        if self.last.as_ref().is_some_and(|last| last.visually_same(state)) {
            return Ok(());
        }
        let palette = &self.palette;
        self.terminal.draw(|frame| crate::ui::draw(frame, state, palette))?;
        self.last = Some(state.clone());
        Ok(())
    }

    /// Fuerza que el siguiente `draw` pinte toda la pantalla.
    pub fn invalidate(&mut self) {
        self.last = None;
        let _ = self.terminal.clear();
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

/// Espera hasta `timeout` por una tecla y la traduce a una acción.
pub fn next_action(timeout: Duration) -> io::Result<Option<Action>> {
    if !event::poll(timeout)? {
        return Ok(None);
    }
    match event::read()? {
        Event::Key(key) => Ok(action_for_key(key)),
        // Al cambiar de tamaño hay que volver a pintar aunque el estado no cambie.
        Event::Resize(..) => Ok(Some(Action::Redraw)),
        _ => Ok(None),
    }
}
