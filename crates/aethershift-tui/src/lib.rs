pub mod app;
pub mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::App;

/// Guard struct to ensure terminal raw mode and alternate screen are restored on drop
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

/// Run the AetherShift TUI console.
pub async fn run_tui(socket_path: Option<PathBuf>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(socket_path);

    // Initial status refresh
    app.refresh().await;

    let tick_rate = Duration::from_millis(500);
    let mut last_tick = std::time::Instant::now();

    while !app.should_quit {
        app.clear_expired_message();

        terminal.draw(|f| ui::draw(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            app.should_quit = true;
                        }
                        KeyCode::Tab => {
                            app.toggle_focus();
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            app.next();
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.previous();
                        }
                        KeyCode::Enter => {
                            app.switch_selected_profile().await;
                        }
                        KeyCode::Char('c') => {
                            app.cycle_profile().await;
                        }
                        KeyCode::Char('r') => {
                            app.restore_baseline().await;
                        }
                        KeyCode::Char('s') => {
                            app.apply_snap().await;
                        }
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.refresh().await;
            last_tick = std::time::Instant::now();
        }
    }

    Ok(())
}
