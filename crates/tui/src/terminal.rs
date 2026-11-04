//! Entrada y salida del modo de terminal de la interfaz.

use crate::input::action_for_key;
use crate::state::Action;
use ratatui::crossterm::event::{self, Event};
use ratatui::DefaultTerminal;
use std::io;
use std::time::Duration;

/// Terminal en modo raw y pantalla alternativa. Al soltarlo (también si hay un
/// pánico) deja la terminal como estaba.
pub struct Tui {
    terminal: DefaultTerminal,
}

impl Tui {
    pub fn enter() -> io::Result<Self> {
        Ok(Self { terminal: ratatui::try_init()? })
    }

    pub fn draw(&mut self, state: &crate::state::UiState) -> io::Result<()> {
        self.terminal.draw(|frame| crate::ui::draw(frame, state)).map(|_| ())
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
        _ => Ok(None),
    }
}
