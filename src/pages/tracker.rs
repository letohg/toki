use itertools::Itertools;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{self, Color, Style, Stylize};
use ratatui::text::{Line, Span, Text, ToLine};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Wrap,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Cell, Paragraph, Row, Table},
};
use std::str::FromStr;

use crate::database::{self, EntityId, TimeEntry};
use crate::pages::tracker::StartTrackingState::ReviewBeforeStart;
use crate::pages::tracker::TrackerAction::{ConsumedKey, StartTracking};
use crate::pages::{PageAction, format_timestamp};
use crate::pages::{delta_hms, tracker};

#[derive(FromPrimitive)]
enum StartTrackingState {
    ProjectSelection,
    TaskSelection,
    EnterDescription,
    ReviewBeforeStart,
}

enum Mode {
    Main,
    StartNewEntry,
    EditActiveEntry,
}

pub enum TrackerAction {
    StartTracking {
        project_id: EntityId,
        description: String,
        task_id: Option<EntityId>,
    },
    UpdateEntry(Option<database::TimeEntry>),
    StopTracking {
        entry_id: EntityId,
    },
    ConsumedKey,
}

struct TrackingInfo {
    duration: String,
    project_name: String,
    project_color: Color,
    task: String,
    description: String,
}

struct NewEntryStartInfo {
    project: Option<database::Project>,
    description: String,
}
impl NewEntryStartInfo {
    pub(crate) fn new() -> Self {
        Self {
            project: None,
            description: String::new(),
        }
    }
}

pub struct Tracker {
    active_entry: Option<database::TimeEntry>,
    recent_entries: Vec<database::TimeEntry>,
    projects: Vec<database::Project>,
    mode: Mode,
    project_list_state: ListState,
    start_new_tracker_list_state: ListState,
    start_tracker_mode: StartTrackingState,
    new_entry_to_start: NewEntryStartInfo,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            active_entry: None,
            recent_entries: Vec::new(),
            projects: Vec::new(),
            mode: Mode::Main,
            project_list_state: ListState::default().with_selected(Some(0)),
            start_new_tracker_list_state: ListState::default().with_selected(Some(0)),
            start_tracker_mode: StartTrackingState::ReviewBeforeStart,
            new_entry_to_start: NewEntryStartInfo::new(),
        }
    }

    pub fn refresh(
        &mut self,
        active_entry: Option<database::TimeEntry>,
        recent_entries: Vec<database::TimeEntry>,

        projects: Vec<database::Project>,
    ) {
        self.active_entry = active_entry;
        self.recent_entries = recent_entries;
        self.projects = projects;

        if let Some(e) = &self.active_entry
            && self.new_entry_to_start.project.is_none()
        {
            self.new_entry_to_start.project = self.projects.iter().find_map(|p| {
                if p.id == e.project_id {
                    return Some(p.clone());
                }
                None
            });
            self.new_entry_to_start.description = e.description.clone();
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.render_main_view(frame, area);
        if matches!(self.mode, Mode::StartNewEntry | Mode::EditActiveEntry)
            && matches!(
                self.start_tracker_mode,
                StartTrackingState::ProjectSelection
            )
        {
            self.render_project_selection_list(frame, area);
        }
    }

    fn render_main_view(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let [tracker_stats_area, entries_area] =
            Layout::vertical([Constraint::Length(6), Constraint::Min(3)]).areas(area);

        let [tracker_area, _, today_stats_area] = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Percentage(50),
        ])
        .areas(tracker_stats_area);
        self.render_tracker(frame, tracker_area);
        self.render_todays_stats(frame, today_stats_area);
        self.render_recent_entries(frame, entries_area);
    }

    fn render_project_selection_list(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let centered_area = area.centered(
            Constraint::Percentage(50),
            if matches!(
                self.start_tracker_mode,
                StartTrackingState::ProjectSelection
            ) {
                Constraint::Percentage(70)
            } else {
                Constraint::Length(9)
            },
        );
        frame.render_widget(Clear, centered_area);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .padding(Padding::symmetric(1, 1))
            .title("Start Tracker");
        frame.render_widget(&block, centered_area);

        let project_list = List::new(self.projects.iter().map(|p| {
            ListItem::new(p.name.as_str())
                .style(Style::new().fg(style::Color::from_str(&p.color).unwrap()))
        }))
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("> ");
        frame.render_stateful_widget(
            project_list,
            block.inner(centered_area),
            &mut self.project_list_state,
        );
    }

    fn render_start_edit_entry_mode(
        &mut self,
        frame: &mut Frame<'_>,
        labels_area: Rect,
        values_area: Rect,
    ) {
        let labels = List::new(vec![
            Line::from("Project:"),
            Line::from("Task:"),
            Line::from("Description:"),
        ]);
        frame.render_stateful_widget(labels, labels_area, &mut self.start_new_tracker_list_state);

        let project_name = create_input_list_item(
            "select project...",
            self.new_entry_to_start
                .project
                .as_ref()
                .map(|p| p.name.clone()),
        );
        let task_name = create_input_list_item("select task...", None);
        let desc = format_as_active_text_input(
            &self.new_entry_to_start.description,
            matches!(
                self.start_tracker_mode,
                StartTrackingState::EnterDescription
            ),
        );

        let description = create_input_list_item("add description...", desc.into());
        frame.render_stateful_widget(
            List::new(vec![project_name, task_name, description])
                .highlight_style(Style::default().bg(Color::DarkGray))
                .highlight_symbol(">"),
            values_area,
            &mut self.start_new_tracker_list_state,
        );
    }

    fn render_recent_entries(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::new().padding(Padding::symmetric(1, 1));
        let [title_area, entries_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(block.inner(area));
        frame.render_widget(Paragraph::new("RECENT ENTRIES"), title_area);
        let table_block = Block::new().borders(Borders::TOP);
        frame.render_widget(&table_block, entries_area);
        let table_area = table_block.inner(entries_area);

        let headings = Row::new(
            ["Project", "Task", "Description", "Time", "Total"]
                .into_iter()
                .map(|t| Cell::from(t).bold()),
        );
        let rows = self.recent_entries.iter().enumerate().map(|(i, e)| {
            let project = self.projects.iter().find(|p| p.id == e.project_id);
            let project_color = project.map_or(style::Color::Yellow, |p| {
                style::Color::from_str(&p.color).unwrap_or_default()
            });
            Row::new(vec![
                Cell::from(project.map_or("None".to_string(), |p| format!("• {}", p.name)))
                    .style(style::Style::new().fg(project_color)),
                "-".into(),
                Cell::from(e.description.clone()),
                Cell::from(format!(
                    "{} - {}",
                    format_timestamp(&e.start_time),
                    e.end_time
                        .map_or("Running".to_owned(), |ts| format_timestamp(&ts))
                )),
                Cell::from(format!("{:>8}", delta_hms(&e.start_time, &e.end_time))),
            ])
            // TODO: better coloring
            .style(style::Style::default().bg(if i % 2 == 0 {
                Color::DarkGray
            } else {
                Color::default()
            }))
        });
        let column_widths = vec![
            Constraint::Length(15),
            Constraint::Min(0),
            Constraint::Min(0),
            Constraint::Length(16), // start time - end time
            Constraint::Length(8),  // duration
        ];
        let table = Table::new(rows, column_widths).header(headings);
        frame.render_widget(table, table_area);
    }

    fn get_active_entry_info(&self) -> Option<TrackingInfo> {
        match &self.active_entry {
            Some(entry) => {
                let project = self.projects.iter().find(|p| p.id == entry.project_id);
                let project_color = project.map_or(style::Color::Yellow, |p| {
                    style::Color::from_str(&p.color).unwrap_or_default()
                });

                let duration = delta_hms(&entry.start_time, &entry.end_time);
                let project_name = project.map_or("None".to_string(), |p| p.name.clone());
                let description = entry.description.clone();
                Some(TrackingInfo {
                    duration,
                    project_name,
                    project_color,
                    task: "-".to_owned(),
                    description,
                })
            }
            None => None,
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<PageAction> {
        let action = match self.mode {
            Mode::Main => self.handle_key_events_in_normal_mode(key_event),
            Mode::StartNewEntry => self.handle_key_events_start_tracking(key_event),
            Mode::EditActiveEntry => self.handle_key_events_start_tracking(key_event),
        };
        action.map_or_default(|a| Some(PageAction::Tracker(a)))
    }

    fn handle_key_events_in_normal_mode(&mut self, key_event: KeyEvent) -> Option<TrackerAction> {
        match key_event.code {
            KeyCode::Char('s') if let Some(entry) = &self.active_entry => {
                self.new_entry_to_start = NewEntryStartInfo::new();
                Some(TrackerAction::StopTracking { entry_id: entry.id })
            }
            KeyCode::Char('s') => {
                self.mode = Mode::StartNewEntry;
                Some(ConsumedKey)
            }
            KeyCode::Char('e') => {
                self.mode = Mode::EditActiveEntry;
                Some(ConsumedKey)
            }
            _ => None,
        }
    }

    fn handle_key_events_start_tracking(&mut self, key_event: KeyEvent) -> Option<TrackerAction> {
        match self.start_tracker_mode {
            StartTrackingState::ProjectSelection => {
                match key_event.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.project_list_state.select_previous();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.project_list_state.select_next();
                    }
                    KeyCode::Enter => {
                        let idx = self.project_list_state.selected().unwrap();
                        let selected_project = self.projects[idx].clone();
                        self.new_entry_to_start.project = Some(selected_project);
                        // TODO: continue with task selection
                        self.start_tracker_mode = StartTrackingState::ReviewBeforeStart;
                    }
                    KeyCode::Esc => self.start_tracker_mode = StartTrackingState::ReviewBeforeStart,
                    _ => (),
                };
                Some(ConsumedKey)
            }
            StartTrackingState::TaskSelection => {
                if let KeyCode::Enter = key_event.code {
                    self.start_new_tracker_list_state.select_next();
                    self.start_tracker_mode = StartTrackingState::ReviewBeforeStart;
                }
                Some(ConsumedKey)
            }
            StartTrackingState::EnterDescription => {
                match key_event.code {
                    KeyCode::Char(c) => {
                        self.new_entry_to_start.description.push(c);
                    }
                    KeyCode::Backspace => {
                        self.new_entry_to_start.description.pop();
                    }
                    KeyCode::Enter | KeyCode::Esc => {
                        self.start_tracker_mode = StartTrackingState::ReviewBeforeStart
                    }
                    _ => (),
                };
                Some(ConsumedKey)
            }
            StartTrackingState::ReviewBeforeStart => match key_event.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.start_new_tracker_list_state.select_previous();
                    Some(ConsumedKey)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.start_new_tracker_list_state.select_next();
                    Some(ConsumedKey)
                }
                KeyCode::Char('s') if self.new_entry_to_start.project.is_some() => {
                    if matches!(self.mode, Mode::StartNewEntry) {
                        self.mode = Mode::Main;
                        return Some(StartTracking {
                            project_id: self.new_entry_to_start.project.as_ref().unwrap().id,
                            description: self.new_entry_to_start.description.clone(),
                            task_id: None,
                        });
                    }
                    Some(TrackerAction::ConsumedKey)
                }
                KeyCode::Char('d') if matches!(self.mode, Mode::EditActiveEntry) => {
                    self.mode = Mode::Main;
                    self.active_entry = match &self.active_entry {
                        Some(e) => Some(TimeEntry {
                            id: e.id,
                            project_id: self.new_entry_to_start.project.clone().unwrap().id,
                            task_id: e.task_id,
                            description: self.new_entry_to_start.description.clone(),
                            start_time: e.start_time,
                            end_time: e.end_time,
                            created_at: e.created_at,
                        }),
                        None => None,
                    };
                    Some(TrackerAction::UpdateEntry(self.active_entry.clone()))
                }
                KeyCode::Enter => {
                    self.start_tracker_mode = StartTrackingState::from_usize(
                        self.start_new_tracker_list_state.selected().unwrap_or(0),
                    )
                    .unwrap();
                    Some(ConsumedKey)
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.mode = Mode::Main;
                    Some(ConsumedKey)
                }
                _ => None,
            },
        }
    }

    fn render_tracker(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let info = self.get_active_entry_info();
        let block = Block::bordered()
            .title(if info.is_some() {
                " TRACKING "
            } else if matches!(self.mode, Mode::StartNewEntry) {
                " STARTING TRACKER "
            } else if matches!(self.mode, Mode::EditActiveEntry) {
                " EDITING TRACKER "
            } else {
                " READY "
            })
            .border_type(ratatui::widgets::BorderType::Rounded)
            .padding(Padding::horizontal(1));
        let tracker_area = block.inner(area);

        let columns = [Constraint::Length(14), Constraint::Min(1)];
        let [time_area, info_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(3)]).areas(tracker_area);
        let [labels_area, values_area] = Layout::horizontal(columns).areas(info_area);

        frame.render_widget(&block, area);

        match self.mode {
            Mode::Main => {
                if info.is_none() {
                    render_no_active_tracker(frame, tracker_area);
                    return;
                }
                render_active_tracker_info(frame, info, time_area, labels_area, values_area);
            }
            Mode::EditActiveEntry | Mode::StartNewEntry => {
                self.render_start_edit_entry_mode(frame, labels_area, values_area);
            }
        };
    }

    fn render_todays_stats(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(" TODAY ")
            .padding(Padding::horizontal(1));
        frame.render_widget(&block, area);
        let stats_area = block.inner(area);
    }

    pub fn get_keybind_hints(&self) -> &str {
        match self.mode {
            Mode::Main => {
                if self.active_entry.is_some() {
                    "Navigate: ⇅  | Stop: s | Edit: e | Quit: q | Switch View: 1/2"
                } else {
                    "Navigate: ⇅  | Start: s | Quit: q | Switch View: 1/2"
                }
            }
            Mode::StartNewEntry => match self.start_tracker_mode {
                StartTrackingState::ProjectSelection => {
                    "Navigate: ⇅  | Start: s | Quit: q | Switch View: 1/2"
                }
                StartTrackingState::TaskSelection => {
                    "Navigate: ⇅  | Start: s | Quit: q | Switch View: 1/2"
                }
                StartTrackingState::EnterDescription => "Submit: <enter>",
                ReviewBeforeStart => {
                    "Navigate: ⇅  | Start: s | Edit: e | Quit: q | Switch View: 1/2"
                }
            },
            Mode::EditActiveEntry => {
                "Navigate: ⇅  | Start/Stop: s | Edit: e | Quit: q | Switch View: 1/2"
            }
        }
    }
}

fn render_active_tracker_info(
    frame: &mut Frame<'_>,
    info: Option<TrackingInfo>,
    time_area: Rect,
    labels_area: Rect,
    values_area: Rect,
) {
    frame.render_widget(
        Paragraph::new(info.as_ref().map_or_default(|i| i.duration.clone()))
            .style(style::Color::Green)
            .centered(),
        time_area,
    );

    let labels = Paragraph::new(vec![
        Line::from("Project:"),
        Line::from("Task:"),
        Line::from("Description:"),
    ]);
    frame.render_widget(labels, labels_area);

    let project_name = Line::from(info.as_ref().map_or_default(|i| i.project_name.clone()))
        .style(info.as_ref().map_or_default(|i| i.project_color));
    let task_name = Line::from(info.as_ref().map_or_default(|i| i.task.clone()));
    let description = Line::from(info.as_ref().map_or_default(|i| i.description.clone()));
    frame.render_widget(
        Paragraph::new(vec![project_name, task_name, description]),
        values_area,
    );
}

fn render_no_active_tracker(frame: &mut Frame<'_>, tracker_area: Rect) {
    frame.render_widget(
        Paragraph::new(vec![
            "No timer is runnig".into(),
            "".into(),
            "Press [s] to start tracking".into(),
        ])
        .wrap(Wrap { trim: true })
        .centered(),
        tracker_area,
    );
}

fn format_as_active_text_input(text: &String, editing_active: bool) -> String {
    format!("{}{}", text, if editing_active { "│" } else { "" })
}

fn create_input_list_item(placeholder: &str, content: Option<String>) -> ListItem<'_> {
    ListItem::new(Line::from(vec![
        "[ ".into(),
        match content {
            None => Span::raw(placeholder).italic(),
            Some(c) if c.is_empty() => Span::raw(placeholder).italic(),
            Some(c) => Span::raw(c),
        },
        Span::raw(" ]"),
    ]))
}
