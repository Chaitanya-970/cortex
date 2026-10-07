//! Keyboard event handling and navigation dispatch for the Cortex TUI.

use crate::app::{ActiveTab, App};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Process a single keyboard event and update [`App`] state accordingly.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            app.should_quit = true;
        }
        KeyCode::Tab => {
            app.next_tab();
        }
        KeyCode::BackTab => {
            app.prev_tab();
        }
        KeyCode::Char('1') => {
            app.set_tab(ActiveTab::Dashboard);
        }
        KeyCode::Char('2') => {
            app.set_tab(ActiveTab::Agents);
        }
        KeyCode::Char('3') => {
            app.set_tab(ActiveTab::ActiveRun);
        }
        KeyCode::Char('4') => {
            app.set_tab(ActiveTab::Events);
        }
        KeyCode::Char('5') => {
            app.set_tab(ActiveTab::History);
        }
        KeyCode::Char('6') => {
            app.set_tab(ActiveTab::Tasks);
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            app.refresh();
            app.status_message = Some("Refreshed state from SQLite store.".to_string());
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.prev_item();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.next_item();
        }
        KeyCode::Enter => {
            app.select_current();
        }
        KeyCode::Esc if app.active_tab != ActiveTab::Dashboard => {
            app.set_tab(ActiveTab::Dashboard);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn make_ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_quit_keys() {
        let mut app = App::new(None);
        assert!(!app.should_quit);

        handle_key(&mut app, make_key(KeyCode::Char('q')));
        assert!(app.should_quit);

        let mut app2 = App::new(None);
        handle_key(&mut app2, make_ctrl_key(KeyCode::Char('c')));
        assert!(app2.should_quit);
    }

    #[test]
    fn test_tab_keys() {
        let mut app = App::new(None);
        assert_eq!(app.active_tab, ActiveTab::Dashboard);

        handle_key(&mut app, make_key(KeyCode::Tab));
        assert_eq!(app.active_tab, ActiveTab::Agents);

        handle_key(&mut app, make_key(KeyCode::BackTab));
        assert_eq!(app.active_tab, ActiveTab::Dashboard);

        handle_key(&mut app, make_key(KeyCode::Char('4')));
        assert_eq!(app.active_tab, ActiveTab::Events);

        handle_key(&mut app, make_key(KeyCode::Esc));
        assert_eq!(app.active_tab, ActiveTab::Dashboard);
    }

    #[test]
    fn test_navigation_keys() {
        let mut app = App::new(None);
        app.set_tab(ActiveTab::Agents);
        assert_eq!(app.selected_agent_idx, 0);

        handle_key(&mut app, make_key(KeyCode::Down));
        assert_eq!(app.selected_agent_idx, 1);

        handle_key(&mut app, make_key(KeyCode::Up));
        assert_eq!(app.selected_agent_idx, 0);
    }
}
