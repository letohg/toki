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
use resolve_path::PathResolveExt;
use std::fs::File;
use std::io::BufReader;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::database;
use crate::database::Timezone;
use crate::pages;
use crate::pages::PageAction;
use crate::pages::format_duration_hms_verbose;
use crate::theme;

#[derive(Debug, PartialEq, EnumIter)]
enum View {
    Tracker,
    Projects,
    Summary,
}

pub enum Page {
    Tracker(pages::tracker::Tracker),
    Projects(pages::projects::Project),
    Summary(pages::summary::Summary),
}

pub struct App {
    db: database::Database,
    exit: bool,
    status: String,
    current_tracker_state_info: Option<(String, chrono::Duration)>,
    active_page: Page,
    error_message: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let database_file = get_active_workspace_db();

        let mut app = Self {
            db: database::Database::new(database_file).unwrap(),
            exit: false,
            status: "Ready".to_string(),
            current_tracker_state_info: None,
            active_page: Page::Tracker(pages::tracker::Tracker::new()),
            error_message: None,
        };
        app.update_state();
        app
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut last_frame_ts = Timezone::now();
        while !self.exit {
            let dt = Timezone::now() - last_frame_ts;
            last_frame_ts = Timezone::now();
            terminal.draw(|frame| self.draw(frame, &dt))?;
            self.handle_events()?;
        }
        Ok(())
    }
    fn update_state(&mut self) {
        let active_entry = self.db.get_running_entry().unwrap_or(None);
        let recent_entries = self.db.get_recent_entries(None, true).unwrap();

        let projects = self.db.get_projects().unwrap(); // TODO: handle failure case
        let metrics = self.db.get_metrics_summary().unwrap(); // TODO: handle failure case

        self.current_tracker_state_info = active_entry.as_ref().map(|e| {
            let name = projects
                .iter()
                .find(|p| p.id == e.project_id)
                .map_or(e.project_id.to_string(), |p| p.name.clone());
            (name, e.duration())
        });

        match &mut self.active_page {
            Page::Tracker(page) => page.refresh(active_entry, recent_entries, projects, metrics),
            Page::Projects(page) => {
                let project_metrics = self.db.get_stats_per_project().unwrap();
                page.refresh(projects, project_metrics);
            }
            Page::Summary(page) => page.refresh(recent_entries, projects, metrics),
        }
    }

    fn draw(&mut self, frame: &mut Frame, dt: &chrono::Duration) {
        // frame.render_widget(
        //     Paragraph::new(vec![
        //         Line::from("ROSEWATER").fg(theme::ROSEWATER),
        //         Line::from("MAUVE").fg(theme::MAUVE),
        //         Line::from("RED").fg(theme::RED),
        //         Line::from("PEACH").fg(theme::PEACH),
        //         Line::from("YELLOW").fg(theme::YELLOW),
        //         Line::from("GREEN").fg(theme::GREEN),
        //         Line::from("TEAL").fg(theme::TEAL),
        //         Line::from("SKY").fg(theme::SKY),
        //         Line::from("BLUE").fg(theme::BLUE),
        //         Line::from("LAVENDER").fg(theme::LAVENDER),
        //         Line::from("TEXT").fg(theme::TEXT),
        //         Line::from("SUBTEXT_1").fg(theme::SUBTEXT_1),
        //         Line::from("SUBTEXT_0").fg(theme::SUBTEXT_0),
        //         Line::from("OVERLAY_2").fg(theme::OVERLAY_2),
        //         Line::from("OVERLAY_1").fg(theme::OVERLAY_1),
        //         Line::from("OVERLAY_0").fg(theme::OVERLAY_0),
        //         Line::from("SURFACE_2").fg(theme::SURFACE_2),
        //         Line::from("SURFACE_1").fg(theme::SURFACE_1),
        //         Line::from("SURFACE_0").fg(theme::SURFACE_0),
        //         Line::from("BASE").fg(theme::BASE),
        //         Line::from("MANTLE").fg(theme::MANTLE),
        //         Line::from("CRUST").fg(theme::CRUST),
        //     ]),
        //     frame.area(),
        // );
        // return;

        let window = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .padding(Padding::symmetric(3, 1))
            .title(" Toki • TimeTracker ")
            .bg(theme::BASE)
            .title_alignment(HorizontalAlignment::Center);
        let area = frame.area();
        frame.render_widget(&window, area);
        let [top, middle, bottom] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .areas(window.inner(area));
        let navbar_block = Block::bordered()
            .borders(Borders::BOTTOM)
            .merge_borders(symbols::merge::MergeStrategy::Exact)
            .style(theme::LAVENDER);

        let footer_block = Block::bordered()
            .borders(Borders::TOP)
            .merge_borders(symbols::merge::MergeStrategy::Exact)
            .style(theme::LAVENDER);
        frame.render_widget(&navbar_block, top);
        self.render_navbar(frame, navbar_block.inner(top));
        self.render_page(frame, middle);
        frame.render_widget(&footer_block, bottom);
        self.render_footer(frame, footer_block.inner(bottom));
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

        match &mut self.active_page {
            Page::Tracker(p) => p.render(frame, area),
            Page::Projects(p) => p.render(frame, area),
            Page::Summary(p) => p.render(frame, area),
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let keybinds = match &self.active_page {
            Page::Tracker(p) => p.get_keybind_hints(),
            Page::Projects(_) => "Select: ⇅  | Quit: q",
            Page::Summary(_) => "Select: ⇅  | Quit: q",
        };
        let [status_area, navigation_area] =
            Layout::horizontal([Constraint::Percentage(25), Constraint::Percentage(75)])
                .areas(area);
        frame.render_widget(
            Paragraph::new(Line::from(self.status.as_str()))
                .left_aligned()
                .fg(theme::PEACH),
            status_area,
        );
        frame.render_widget(
            Paragraph::new(keybinds)
                .right_aligned()
                .fg(theme::OVERLAY_0),
            navigation_area,
        );
    }

    fn render_navbar(&self, frame: &mut Frame, area: Rect) {
        let [tabs_area, appname_area] =
            Layout::horizontal([Constraint::Min(25), Constraint::Min(25)]).areas(area);

        let views: Vec<Span> = View::iter()
            .enumerate()
            .map(|(i, view)| -> Span {
                let is_active = matches!(
                    (&self.active_page, &view),
                    (&Page::Tracker(_), &View::Tracker)
                        | (&Page::Projects(_), &View::Projects)
                        | (&Page::Summary(_), &View::Summary)
                );
                let label = format!("[{}] {:?}", i + 1, view);
                if !is_active {
                    return Span::styled(
                        label,
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(theme::OVERLAY_0),
                    );
                }
                Span::styled(
                    label,
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(theme::GREEN),
                )
            })
            .collect::<Vec<Span>>()
            .into_iter()
            .intersperse(" ".into())
            .collect();

        frame.render_widget(Paragraph::new(Line::from(views)), tabs_area);
        frame.render_widget(
            Paragraph::new(self.current_tracker_state_info.as_ref().map_or(
                Line::from("  idle").fg(theme::GREEN),
                |(p, d)| {
                    Line::from(vec![
                        Span::from(" ").fg(theme::RED),
                        Span::from(format!("{p} - {}", format_duration_hms_verbose(d)))
                            .bold()
                            .fg(theme::TEXT),
                    ])
                },
            ))
            .right_aligned(),
            appname_area,
        );
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
        if self.handle_current_page_keyevent(key_event)? {
            return Ok(());
        }

        if self.handle_application_keyevent(key_event) {
            self.update_state();
        }

        Ok(())
    }

    fn handle_application_keyevent(&mut self, key_event: KeyEvent) -> bool {
        match key_event.code {
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
            KeyCode::Char('3') => {
                self.active_page = Page::Summary(pages::summary::Summary::new());
                true
            }
            _ => false,
        }
    }

    /// True means key was consumed, false means it should be passed to the application level
    /// handler
    fn handle_current_page_keyevent(
        &mut self,
        key_event: KeyEvent,
    ) -> Result<bool, color_eyre::eyre::Error> {
        match &mut self.active_page {
            Page::Tracker(p) => {
                if let Some(action) = p.handle_key_event(key_event) {
                    return self.handle_page_action(action);
                }
                Ok(false)
            }
            Page::Projects(p) => {
                if let Some(action) = p.handle_key_event(key_event) {
                    return self.handle_page_action(action);
                }
                Ok(false)
            }
            Page::Summary(p) => Ok(p.handle_key_event(key_event)),
        }
    }

    fn handle_page_action(&mut self, action: PageAction) -> Result<bool, color_eyre::eyre::Error> {
        Ok(match action {
            PageAction::Tracker(a) => match a {
                pages::tracker::TrackerAction::StartTracking {
                    project_id,
                    description,
                    task_id,
                } => {
                    let _ = self.db.start_entry(project_id, description, task_id);
                    self.status = "Timer started".to_string();
                    self.update_state();
                    true
                }
                pages::tracker::TrackerAction::UpdateEntry(entry) => {
                    self.db.update_entry(entry)?;
                    self.status = "Timer updated".to_string();
                    self.update_state();
                    true
                }
                pages::tracker::TrackerAction::StopTracking { entry_id } => {
                    let _ = self.db.stop_entry(entry_id);
                    self.status = "Timer stopped".to_string();
                    self.update_state();
                    true
                }
                // TODO: ensure that this causes app to not check for keys
                pages::tracker::TrackerAction::ConsumedKey => true,
            },
            PageAction::Projects(a) => match a {
                pages::projects::ProjectAction::CreateProject { name, description } => {
                    let _ = self.db.create_project(&name, "#4A90D9", &description);
                    self.update_state();
                    true
                }
                pages::projects::ProjectAction::UpdateProject {
                    id,
                    name,
                    description,
                } => {
                    let _ = self.db.update_project(id, &name, "#4A90D9", &description);
                    self.update_state();
                    true
                }
            },
            _ => false,
        })
    }

    fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
    }
}

fn get_active_workspace_db() -> String {
    const CONFIG_DIR: &str = "~/.time_tracker";
    let settings_file = format!("{}/settings.json", CONFIG_DIR);

    if matches!(std::fs::exists(&settings_file), Ok(true)) {
        let file = File::open(settings_file.resolve()).unwrap();
        let reader = BufReader::new(file);
        let settings: serde_json::Value = serde_json::from_reader(reader).unwrap();
        let id = settings["active_workspace_id"].as_str().unwrap();
        format!("{CONFIG_DIR}/workspaces/{}.db", id)
    } else {
        format!("{CONFIG_DIR}/workspaces/default.db")
    }
}
