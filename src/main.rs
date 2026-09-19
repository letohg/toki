mod database;

use color_eyre::eyre::Result;
use core::time::Duration;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::DisableMouseCapture;
use ratatui::crossterm::event::EnableMouseCapture;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use ratatui::prelude::*;
use ratatui::widgets::BorderType;
use ratatui::widgets::Table;
use ratatui::widgets::{List, Row};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::Span,
    widgets::Block,
    widgets::Paragraph,
};
use std::io;

fn delta_hms(start: &str, end: &Option<String>) -> String {
    let ts = chrono::DateTime::parse_from_rfc3339(start).unwrap();
    let e = if let Some(e_ts) = end {
        chrono::DateTime::parse_from_rfc3339(e_ts)
            .unwrap()
            .with_timezone(&chrono::Utc)
    } else {
        chrono::Utc::now()
    };

    let delta: chrono::TimeDelta = e - ts.with_timezone(&chrono::Utc);
    let h = delta.num_hours();
    let m = delta.num_minutes() % 60;
    let s = delta.num_seconds() % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

struct App {
    exit: bool,
    active_entry: Option<database::TimeEntry>,
    recent_entries: Vec<database::TimeEntry>,
}

impl App {
    pub fn new() -> Self {
        let active_entry = database::get_running_entry();
        let recent_entries = database::get_recent_entries(10);
        Self {
            exit: false,
            active_entry,
            recent_entries,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }
    fn update_state(&mut self) {
        self.active_entry = database::get_running_entry();
        self.recent_entries = database::get_recent_entries(10);
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
        self.render_tracker_view(frame, page.inner(middle));
    }

    fn render_tracker_view(&self, frame: &mut Frame, area: Rect) {
        let [tracker_area, entries_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        let tracker_text = if let Some(entry) = &self.active_entry {
            format!(
                "Tracking: {} | {} | {}",
                entry.project_id,
                entry.description,
                delta_hms(&entry.start_time, &entry.end_time)
            )
        } else {
            "No running entry".to_string()
        };

        let p = Paragraph::new(tracker_text)
            .style(style::Color::Yellow)
            .left_aligned();
        frame.render_widget(p, tracker_area);

        let rows = self.recent_entries.iter().map(|e| {
            Row::new(vec![
                format!("{}", e.project_id),
                format!("{}", e.task_id.unwrap_or(0)),
                format!("{}", e.description),
                format!("{}", e.start_time),
                format!("{}", e.end_time.clone().unwrap_or("Running".to_owned())),
                format!("{}", delta_hms(&e.start_time, &e.end_time)),
            ])
        });
        let table = Table::default().rows(rows);
        frame.render_widget(table, entries_area);
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
                if let Some(entry) = &self.active_entry {
                    match database::stop_entry(entry.id) {
                        Ok(_) => (),
                        Err(_) => {
                            println!("The last know running entries has been stopped already")
                        }
                    };
                    self.active_entry = None;
                } else {
                    self.active_entry =
                        Some(database::start_entry(1, "From Rust".to_string(), None).unwrap());
                }
            }
            _ => (),
        }
        self.update_state();
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

    match App::new().run(&mut terminal) {
        Ok(_) => (),
        Err(_) => todo!(),
    };

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
