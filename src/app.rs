use color_eyre::eyre::Result;
use core::time::Duration;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Padding;
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
use crate::pages;

#[derive(Debug, PartialEq, EnumIter)]
enum View {
    Tracker,
    Projects,
}

pub enum Page {
    Tracker(pages::tracker::Tracker),
    Projects(pages::projects::Project),
}

pub struct App {
    db: database::Database,
    exit: bool,
    active_page: Page,
    error_message: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            db: database::Database::new().unwrap(),
            exit: false,
            active_page: Page::Tracker(pages::tracker::Tracker::new()),
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
        let active_entry = self.db.get_running_entry();
        let recent_entries = self.db.get_recent_entries(10);

        let projects = self.db.get_projects().unwrap(); // TODO: handle failure case

        match &mut self.active_page {
            Page::Tracker(page) => page.refresh(active_entry, recent_entries, projects),
            Page::Projects(page) => page.refresh(projects),
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let window = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .padding(Padding::horizontal(1))
            .title("Toki")
            .title_alignment(HorizontalAlignment::Center);
        let area = frame.area();
        frame.render_widget(&window, area);
        let [top, middle, bottom] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .areas(window.inner(area));

        self.render_navbar(frame, top);
        self.render_page(frame, middle);
        self.render_footer(frame, bottom);
        self.render_popup(frame, window.inner(area));
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
        // let page = Block::bordered()
        //     // .border_type(BorderType::Rounded)
        //     .borders(Borders::BOTTOM | Borders::TOP)
        //     .title("Timer");
        // frame.render_widget(&page, area);

        match &self.active_page {
            Page::Tracker(p) => p.render(frame, area),
            Page::Projects(p) => p.render(frame, area),
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        // TODO: move into page render logic
        let keybinds = match &self.active_page {
            Page::Tracker(_) => {
                "Navigate: ⇅  | Start/Stop: s | Edit: e | Quit: q | Switch View: 1/2"
            }
            Page::Projects(_) => "Navigate: ⇅  | Quit: q | Switch View: 1/2",
        };
        let [_, bottom] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);
        let footer = Paragraph::new(keybinds);
        frame.render_widget(footer, bottom);
    }

    fn render_navbar(&self, frame: &mut Frame, top: Rect) {
        // let navbar = Block::new().borders(Borders::BOTTOM);

        let views: Vec<Span> = View::iter()
            .enumerate()
            .map(|(i, view)| -> Span {
                let is_active = matches!(
                    (&self.active_page, &view),
                    (&Page::Tracker(_), &View::Tracker) | (&Page::Projects(_), &View::Projects)
                );
                let label = format!("[{}] {:?}", i + 1, view);
                if !is_active {
                    return label.into();
                }
                Span::styled(
                    label,
                    Style::default().add_modifier(Modifier::BOLD).light_yellow(),
                )
            })
            .collect::<Vec<Span>>()
            .into_iter()
            .intersperse(" ".into())
            .collect();
        frame.render_widget(Paragraph::new(Line::from(views)), top);
    }

    fn handle_events(&mut self) -> Result<()> {
        let timeout = Duration::from_secs_f64(1.0 / 50.0);
        if !event::poll(timeout)? {
            return Ok(());
        }
        if let Event::Key(key_event) = event::read()? {
            self.handle_key_event(key_event)?
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        let mut need_state_update: bool = match key_event.code {
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
                self.active_page = Page::Tracker(pages::tracker::Tracker::new());
                true
            }
            KeyCode::Char('2') => {
                self.active_page = Page::Projects(pages::projects::Project::new());
                true
            }
            // KeyCode::Char('s') => {}
            _ => false,
        };
        need_state_update |= match &mut self.active_page {
            Page::Tracker(p) => match p.handle_key_event(key_event) {
                Some(action) => {
                    match action {
                        pages::tracker::TrackerAction::StartTracking(
                            project_id,
                            description,
                            task_id,
                        ) => {
                            let _ = self.db.start_entry(project_id, description, task_id);
                        }
                        pages::tracker::TrackerAction::StopTracking(entry_id) => {
                            let _ = self.db.stop_entry(entry_id);
                        }
                    };
                    true
                }
                None => false,
            },
            Page::Projects(p) => p.handle_key_event(key_event),
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
