mod database;
use color_eyre::eyre::Result;
use core::time::Duration;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use itertools::Itertools;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::DisableMouseCapture;
use ratatui::crossterm::event::EnableMouseCapture;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use ratatui::prelude::*;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Cell;
use ratatui::widgets::Clear;
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
use std::str::FromStr;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::database::{EntityId, Timestamp, Timezone};

fn delta_hms(start: &Timestamp, end: &Option<Timestamp>) -> String {
    let delta: chrono::TimeDelta = end.unwrap_or(Timezone::now()) - start;
    let h = delta.num_hours();
    let m = delta.num_minutes() % 60;
    let s = delta.num_seconds() % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn format_timestamp(ts: &Timestamp) -> String {
    ts.format("%H:%M").to_string()
}

#[derive(Debug, PartialEq, EnumIter)]
enum View {
    Timer,
    Projects,
}

struct App {
    exit: bool,
    active_view: View,
    active_entry: Option<database::TimeEntry>,
    recent_entries: Vec<database::TimeEntry>,
    projects: Vec<database::Project>,
    error_message: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            exit: false,
            active_view: View::Timer,
            active_entry: None,
            recent_entries: vec![],
            projects: vec![],
            error_message: None,
        };
        app.update_state();
        app
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
        match database::get_projects() {
            Ok(projects) => {
                self.projects = projects;
            }
            Err(_) => {
                self.set_error("Failed to retrieve Projects from Database!".to_owned());
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let window = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("TimeTracker");
        let area = frame.area();
        frame.render_widget(&window, area);
        let [top, middle, bottom] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(window.inner(area));

        self.render_navbar(frame, top);
        self.render_page(frame, middle);
        self.render_footer(frame, bottom);
        self.render_popup(frame, area);
    }

    fn render_popup(&mut self, frame: &mut Frame, area: Rect) {
        if let Some(error) = self.error_message.as_ref() {
            let popup_block = Block::bordered().title("Error").style(Color::Red);
            let centered_area =
                area.centered(Constraint::Percentage(60), Constraint::Percentage(20));
            frame.render_widget(Clear, centered_area);
            let error_msg = Paragraph::new(error.clone()).block(popup_block).bold();
            frame.render_widget(error_msg, centered_area);
        }
    }

    fn render_page(&mut self, frame: &mut Frame, area: Rect) {
        let page = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Tracker");
        frame.render_widget(&page, area);

        match self.active_view {
            View::Timer => self.render_tracker_view(frame, page.inner(area)),
            View::Projects => self.render_project_view(frame, page.inner(area)),
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let keybinds = match self.active_view {
            View::Timer => "Navigate: ⇅  | Start/Stop: s | Edit: e | Quit: q | Switch View: 1/2",
            View::Projects => "Navigate: ⇅  | Quit: q | Switch View: 1/2",
        };
        let footer = Paragraph::new(keybinds);
        frame.render_widget(footer, area);
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
            let project = self.projects.iter().find(|p| p.id == e.project_id);
            let project_color = project.map_or(style::Color::Yellow, |p| {
                style::Color::from_str(&p.color).unwrap()
            });
            Row::new(vec![
                Cell::from("*").style(Style::new().fg(project_color)),
                Cell::from(project.map_or("None".to_string(), |p| p.name.clone())),
                Cell::from(e.description.clone()),
                Cell::from(format!(
                    "{} - {}",
                    format_timestamp(&e.start_time),
                    e.end_time
                        .map_or("Running".to_owned(), |ts| format_timestamp(&ts))
                )),
                Cell::from(delta_hms(&e.start_time, &e.end_time)),
            ])
        });
        let column_widths = vec![
            Constraint::Length(1),
            Constraint::Length(15),
            Constraint::Min(0),
            Constraint::Length(16), // start time - end time
            Constraint::Length(8),  // duration
        ];
        let table = Table::new(rows, column_widths);
        frame.render_widget(table, entries_area);
    }

    fn render_navbar(&self, frame: &mut Frame, top: Rect) {
        let navbar = Block::new().borders(Borders::BOTTOM);

        let views: Vec<Span> = View::iter()
            .enumerate()
            .map(|(i, view)| {
                if self.active_view == view {
                    Span::styled(
                        format!("[{}] {:?}", i + 1, view),
                        Style::default().add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::raw(format!("[{}] {:?}", i + 1, view))
                }
            })
            .collect::<Vec<Span>>()
            .into_iter()
            .intersperse(Span::raw(" "))
            .collect();
        frame.render_widget(Paragraph::new(Line::from(views)).block(navbar), top);
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
        let need_state_update: bool = match key_event.code {
            KeyCode::Char('q') => {
                if self.error_message.is_some() {
                    self.error_message = None;
                    true
                } else {
                    self.exit = true;
                    false
                }
            }
            KeyCode::Char('1') => {
                self.active_view = View::Timer;
                false
            }
            KeyCode::Char('2') => {
                self.active_view = View::Projects;
                false
            }
            KeyCode::Char('s') => {
                if let Some(entry) = &self.active_entry {
                    match database::stop_entry(entry.id) {
                        Ok(_) => {
                            self.active_entry = None;
                            true
                        }
                        Err(_) => {
                            self.error_message = Some(
                                "The last know running entries has been stopped already".to_owned(),
                            );
                            false
                        }
                    }
                } else {
                    self.active_entry =
                        match database::start_entry(1, "From Rust".to_string(), None) {
                            Ok(e) => Some(e),
                            Err(_) => {
                                self.error_message = Some(
                                    "The last know running entries has been stopped already"
                                        .to_owned(),
                                );
                                None
                            }
                        };
                    self.active_entry.is_some()
                }
            }
            _ => false,
        };
        if need_state_update {
            self.update_state();
        }
        Ok(())
    }

    fn render_project_view(&mut self, frame: &mut Frame, area: Rect) {
        let rows = self.projects.iter().map(|p| {
            Row::new(vec![
                Cell::from("*").style(Style::new().fg(style::Color::from_str(&p.color).unwrap())),
                Cell::from(p.name.clone()),
                Cell::from(p.description.clone()),
            ])
        });
        let column_widths = vec![
            Constraint::Length(1),
            Constraint::Length(15),
            Constraint::Min(0),
        ];
        let table = Table::new(rows, column_widths);
        frame.render_widget(table, area);
    }

    fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
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
