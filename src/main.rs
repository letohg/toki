use color_eyre::eyre::Result;
use core::time::Duration;
use ratatui::widgets::BorderType;
use std::io;

mod database;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::DisableMouseCapture;
use ratatui::crossterm::event::EnableMouseCapture;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use ratatui::prelude::*;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::Span,
    widgets::Block,
    widgets::Paragraph,
};

fn delta_hms(timestamp: &str) -> String {
    let ts = chrono::DateTime::parse_from_rfc3339(timestamp).unwrap();
    let now = chrono::Utc::now();
    let delta: chrono::TimeDelta = now - ts.with_timezone(&chrono::Utc);
    let h = delta.num_hours();
    let m = delta.num_minutes() % 60;
    let s = delta.num_seconds() % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

struct App {
    exit: bool,
    active_entry: Option<database::TimeEntry>,
}

impl App {
    pub fn new() -> Self {
        Self {
            exit: false,
            active_entry: None,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit {
            self.active_entry = database::Database::new().get_running_entry();
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }
    fn draw(&self, frame: &mut Frame) {
        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("TimeTracker");
        let area = frame.area();
        let inner_area = block.inner(area);
        let [top, middle, bottom] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .areas(inner_area);

        let navbar = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Navigation");
        let text = Self::create_navbar(vec!["Home", "Projects"]);
        frame.render_widget(Paragraph::new(text).block(navbar), top);

        let footer = Block::bordered().border_type(BorderType::Rounded);
        frame.render_widget(footer, bottom);

        let page = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Tracker");
        frame.render_widget(&page, middle);
        if let Some(entry) = &self.active_entry {
            let text = format!(
                "Tracking: {} | {} | {}",
                entry.project_id,
                entry.description,
                delta_hms(&entry.start_time)
            );
            let p = Paragraph::new(text)
                .style(style::Color::Yellow)
                .left_aligned();
            frame.render_widget(p, page.inner(middle));
        } else {
            let p = Paragraph::new("No running entry")
                .style(style::Color::Yellow)
                .left_aligned();
            frame.render_widget(p, page.inner(middle));
        }
    }

    fn create_navbar(items: Vec<&str>) -> Line {
        let styled_items: Vec<Span> = items
            .iter()
            .map(|item| {
                let mut chars = item.chars();
                let first_char = chars.next().unwrap_or(' ');
                let rest: String = chars.collect();
                vec![
                    Span::styled(
                        first_char.to_string(),
                        Style::default().add_modifier(Modifier::UNDERLINED),
                    ), // Underlined letter
                    Span::raw(rest),
                    Span::raw(" | ".to_string()),
                ]
            })
            .flatten()
            .collect();
        Line::from(styled_items)
    }
    fn handle_events(&mut self) -> Result<()> {
        let timeout = Duration::from_secs_f64(1.0 / 50.0);
        if !event::poll(timeout)? {
            return Ok(());
        }
        match event::read()? {
            Event::Key(key_event) => self.handle_key_event(key_event)?,
            _ => (),
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Char('q') => {
                println!("Exiting");
                self.exit = true;
            }
            KeyCode::Char('s') => {
                let db = database::Database::new();
                if let Some(entry) = &self.active_entry {
                    db.stop_entry(entry.id);
                    self.active_entry = None;
                } else {
                    self.active_entry =
                        Some(db.start_entry(1, "From Rust".to_string(), None).unwrap());
                }
            }
            _ => (),
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout(); // This is a special case. Normally using stdout is fine
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    App::new().run(&mut terminal)?;

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
