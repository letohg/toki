use color_eyre::eyre::Result;
use core::time::Duration;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::Span,
    widgets::Block,
    widgets::Paragraph,
};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::database;
use crate::views;

#[derive(Debug, PartialEq, EnumIter)]
enum View {
    Timer,
    Projects,
}

pub struct App {
    db: database::Database,
    exit: bool,
    active_view: View,
    active_entry: Option<database::TimeEntry>,
    error_message: Option<String>,
    // views
    timer_view: views::timer::Timer,
    project_view: views::projects::Project,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            db: database::Database::new().unwrap(),

            exit: false,
            active_view: View::Timer,
            active_entry: None,
            error_message: None,
            timer_view: views::timer::Timer::new(),
            project_view: views::projects::Project::new(),
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
        let active_entry = self.db.get_running_entry();
        let recent_entries = self.db.get_recent_entries(10);

        let projects = self.db.get_projects().unwrap(); // TODO: handle failure case

        match self.active_view {
            View::Timer => self
                .timer_view
                .refresh(active_entry, recent_entries, projects),
            View::Projects => self.project_view.refresh(projects),
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
            // .border_type(BorderType::Rounded)
            .borders(Borders::BOTTOM | Borders::TOP)
            .title("Timer");
        frame.render_widget(&page, area);

        match self.active_view {
            View::Timer => {
                self.timer_view.render(frame, page.inner(area));
            }
            View::Projects => self.project_view.render(frame, page.inner(area)),
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
                true
            }
            KeyCode::Char('2') => {
                self.active_view = View::Projects;
                true
            }
            KeyCode::Char('s') => {
                if let Some(entry) = &self.active_entry {
                    match self.db.stop_entry(entry.id) {
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
                    self.active_entry = match self.db.start_entry(1, "From Rust".to_string(), None)
                    {
                        Ok(e) => Some(e),
                        Err(_) => {
                            self.error_message = Some(
                                "The last know running entries has been stopped already".to_owned(),
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

    fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
    }
}
