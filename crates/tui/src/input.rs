//! Teclas → acciones. El joystick producirá las mismas [`Action`].
//!
//! | Tecla | Acción |
//! |-------|--------|
//! | F1 / F2, Tab | Pantalla Play / Synth / siguiente |
//! | ↑ ↓ (k j) | Parámetro anterior / siguiente |
//! | ← → (h l) | Ajuste normal (2 %) |
//! | Mayús + ← →, `,` `.` | Ajuste fino (0,4 %) |
//! | RePág / AvPág | Ajuste grueso (10 %) |
//! | F5 | Pantalla Files |
//! | Enter, `s`, `n` | En Files: cargar, guardar sobre el seleccionado, guardar nuevo |
//! | Ctrl+L | Repintar la pantalla |
//! | q, Ctrl+C | Salir |
//!
//! El ajuste fino también está en `,` y `.` porque la consola del kernel no
//! distingue Mayús + flechas.

use crate::state::{Action, Screen, Step};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub fn action_for_key(key: KeyEvent) -> Option<Action> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let horizontal = if shift { Step::Fine } else { Step::Normal };
    match key.code {
        KeyCode::Char('c') if ctrl => Some(Action::Quit),
        KeyCode::Char('l') if ctrl => Some(Action::Redraw),
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::F(1) => Some(Action::Screen(Screen::Play)),
        KeyCode::F(2) => Some(Action::Screen(Screen::Synth)),
        KeyCode::F(5) => Some(Action::Screen(Screen::Files)),
        KeyCode::Tab => Some(Action::NextScreen),
        KeyCode::Enter => Some(Action::Confirm),
        KeyCode::Char('s') => Some(Action::Save),
        KeyCode::Char('n') => Some(Action::SaveNew),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::Down),
        KeyCode::Left | KeyCode::Char('h') => Some(Action::Adjust { direction: -1, step: horizontal }),
        KeyCode::Right | KeyCode::Char('l') => Some(Action::Adjust { direction: 1, step: horizontal }),
        KeyCode::Char(',') => Some(Action::Adjust { direction: -1, step: Step::Fine }),
        KeyCode::Char('.') => Some(Action::Adjust { direction: 1, step: Step::Fine }),
        KeyCode::PageDown => Some(Action::Adjust { direction: -1, step: Step::Coarse }),
        KeyCode::PageUp => Some(Action::Adjust { direction: 1, step: Step::Coarse }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventKind;

    fn press(code: KeyCode) -> Option<Action> {
        action_for_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn function_keys_pick_screens() {
        assert_eq!(press(KeyCode::F(1)), Some(Action::Screen(Screen::Play)));
        assert_eq!(press(KeyCode::F(2)), Some(Action::Screen(Screen::Synth)));
        assert_eq!(press(KeyCode::Tab), Some(Action::NextScreen));
    }

    #[test]
    fn arrows_and_vim_keys_navigate_and_adjust() {
        assert_eq!(press(KeyCode::Up), press(KeyCode::Char('k')));
        assert_eq!(press(KeyCode::Down), press(KeyCode::Char('j')));
        assert_eq!(press(KeyCode::Right), Some(Action::Adjust { direction: 1, step: Step::Normal }));
        assert_eq!(press(KeyCode::Char('h')), Some(Action::Adjust { direction: -1, step: Step::Normal }));
    }

    #[test]
    fn modifier_and_alternative_keys_give_fine_and_coarse_steps() {
        let shift_right = action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
        assert_eq!(shift_right, Some(Action::Adjust { direction: 1, step: Step::Fine }));
        assert_eq!(press(KeyCode::Char('.')), Some(Action::Adjust { direction: 1, step: Step::Fine }));
        assert_eq!(press(KeyCode::Char(',')), Some(Action::Adjust { direction: -1, step: Step::Fine }));
        assert_eq!(press(KeyCode::PageUp), Some(Action::Adjust { direction: 1, step: Step::Coarse }));
        assert_eq!(press(KeyCode::PageDown), Some(Action::Adjust { direction: -1, step: Step::Coarse }));
    }

    #[test]
    fn files_screen_keys() {
        assert_eq!(press(KeyCode::F(5)), Some(Action::Screen(Screen::Files)));
        assert_eq!(press(KeyCode::Enter), Some(Action::Confirm));
        assert_eq!(press(KeyCode::Char('s')), Some(Action::Save));
        assert_eq!(press(KeyCode::Char('n')), Some(Action::SaveNew));
    }

    #[test]
    fn ctrl_l_redraws_but_plain_l_still_adjusts() {
        let ctrl_l = action_for_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        assert_eq!(ctrl_l, Some(Action::Redraw));
        assert_eq!(press(KeyCode::Char('l')), Some(Action::Adjust { direction: 1, step: Step::Normal }));
    }

    #[test]
    fn q_and_ctrl_c_quit() {
        assert_eq!(press(KeyCode::Char('q')), Some(Action::Quit));
        let ctrl_c = action_for_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(ctrl_c, Some(Action::Quit));
        assert_eq!(press(KeyCode::Char('c')), None);
    }

    #[test]
    fn releases_and_unknown_keys_are_ignored() {
        let mut release = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(action_for_key(release), None);
        assert_eq!(press(KeyCode::Char('z')), None);
    }
}
