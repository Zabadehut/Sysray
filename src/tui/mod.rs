pub mod dashboard;
pub mod theme;
pub mod widgets;

use crate::collectors::Snapshot;
use crate::config::TuiConfig;
use crate::engine::scheduler::TickEvent;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dashboard::{Dashboard, Panel};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use tokio::sync::broadcast;

pub async fn run_tui(config: &TuiConfig, mut rx: broadcast::Receiver<TickEvent>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut dashboard = Dashboard::new(&config.theme);
    let refresh = Duration::from_millis(config.refresh_rate_ms);
    let mut current_snapshot = Snapshot::default();

    loop {
        // Drainer la queue et garder uniquement le snapshot le plus récent
        loop {
            match rx.try_recv() {
                Ok(tick) => current_snapshot = tick.snapshot,
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Lagged(_)) => break,
                Err(broadcast::error::TryRecvError::Closed) => {
                    restore_terminal(&mut terminal)?;
                    return Ok(());
                }
            }
        }

        terminal.draw(|frame| dashboard.render(frame, &current_snapshot))?;

        if event::poll(refresh)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => break,
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        terminal.clear()?;
                    }
                    KeyCode::Char('t') | KeyCode::Char('T') => {
                        dashboard.cycle_theme();
                        terminal.clear()?;
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        dashboard.toggle_panel(Panel::Cpu);
                        terminal.clear()?;
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        dashboard.toggle_panel(Panel::Memory);
                        terminal.clear()?;
                    }
                    KeyCode::Char('l') | KeyCode::Char('L') => {
                        dashboard.toggle_panel(Panel::Linux);
                        terminal.clear()?;
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        dashboard.toggle_panel(Panel::Disk);
                        terminal.clear()?;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        dashboard.toggle_panel(Panel::Network);
                        terminal.clear()?;
                    }
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        dashboard.toggle_panel(Panel::Alerts);
                        terminal.clear()?;
                    }
                    KeyCode::Char('p') | KeyCode::Char('P') => {
                        dashboard.toggle_panel(Panel::Process);
                        terminal.clear()?;
                    }
                    _ => {}
                }
            }
        }
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
