use std::io::{self, IsTerminal};

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
use serde_json::json;

use crate::{daemon, home::Home};

pub fn run(home: Home) -> crate::Result<()> {
    home.require_initialized()?;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        println!(
            "zgent TUI requires an interactive terminal. Start the coordinator with `zgent daemon serve` and run `zgent` in a TTY."
        );
        return Ok(());
    }

    let socket = daemon::socket_path(&home, None);
    let daemon_status = match daemon::send_request(socket, json!({ "command": "health" })) {
        Ok(response) if response["ok"] == true => "daemon: connected".to_string(),
        Ok(response) => format!(
            "daemon: {}",
            response["error"].as_str().unwrap_or("unhealthy")
        ),
        Err(_) => "daemon: not running; start `zgent daemon serve`".to_string(),
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                ])
                .split(frame.area());

            frame.render_widget(
                Paragraph::new(vec![
                    Line::from("zgent chatbox"),
                    Line::from(daemon_status.as_str()),
                    Line::from("Static adapters are loaded from .zgent/adapters/*.toml"),
                ])
                .block(Block::default().title("Session").borders(Borders::ALL)),
                chunks[0],
            );
            frame.render_widget(
                Paragraph::new("review-first | yolo").block(
                    Block::default()
                        .title("Permission Mode")
                        .borders(Borders::ALL),
                ),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new("Press q to quit")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(Block::default().title("Input").borders(Borders::ALL)),
                chunks[2],
            );
        })?;

        if let Event::Key(key) = event::read()?
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
