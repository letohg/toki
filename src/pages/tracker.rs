use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{self, Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Wrap,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Cell, Paragraph, Row, Table},
};
use std::str::FromStr;

use crate::database::{self, EntityId, TimeEntry, Timestamp, Timezone};
use crate::pages::delta_hms;
use crate::pages::tracker::StartOrEditEntryState::ReviewBeforeStart;
use crate::pages::tracker::TrackerAction::StartTracking;
use crate::pages::{PageAction, format_delta_time_statistic, format_timestamp};

#[derive(Clone, FromPrimitive)]
enum StartOrEditEntryState {
    ProjectSelection,
    TaskSelection,
    EnterDescription,
    ReviewBeforeStart,
}

#[derive(Clone)]
struct StartNewEntryState {
    new_entry: NewEntryStartInfo,
    mode: StartOrEditEntryState,
    menu_list_state: ListState,
    project_list_state: ListState,
}
impl StartNewEntryState {
    fn default() -> Self {
        Self {
            new_entry: NewEntryStartInfo::default(),
            mode: StartOrEditEntryState::ReviewBeforeStart,
            menu_list_state: ListState::default().with_selected(Some(0)),
            project_list_state: ListState::default().with_selected(Some(0)),
        }
    }
}

#[derive(Clone)]
struct EditActiveEntryState {
    active_entry: database::TimeEntry,
    mode: StartOrEditEntryState,
    menu_list_state: ListState,
    project_list_state: ListState,
}
impl EditActiveEntryState {
    fn default() -> Self {
        Self {
            active_entry: TimeEntry::default(),
            mode: StartOrEditEntryState::ReviewBeforeStart,
            menu_list_state: ListState::default().with_selected(Some(0)),
            project_list_state: ListState::default().with_selected(Some(0)),
        }
    }
}

#[derive(Clone)]
enum TimerMode {
    NoRunningEntry,
    ShowActiveEntry(database::TimeEntry),
    StartNewEntry(StartNewEntryState),
    EditActiveEntry(EditActiveEntryState),
}

pub enum TrackerAction {
    StartTracking {
        project_id: EntityId,
        description: String,
        task_id: Option<EntityId>,
    },
    UpdateEntry(database::TimeEntry),
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

#[derive(Default, Clone)]
struct NewEntryStartInfo {
    project: Option<database::Project>,
    description: String,
}

pub struct Tracker {
    now: Timestamp,
    recent_entries: Vec<database::TimeEntry>,
    projects: Vec<database::Project>,
    timer_mode: TimerMode,
    todays_statistic: database::DailyStats,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            now: Timezone::now(),
            recent_entries: Vec::new(),
            projects: Vec::new(),
            timer_mode: TimerMode::NoRunningEntry,
            todays_statistic: database::DailyStats::default(),
        }
    }

    pub fn refresh(
        &mut self,
        active_entry: Option<database::TimeEntry>,
        recent_entries: Vec<database::TimeEntry>,

        projects: Vec<database::Project>,
        todays_statistic: database::DailyStats,
    ) {
        if let Some(e) = active_entry {
            self.timer_mode = TimerMode::ShowActiveEntry(e);
        }

        self.recent_entries = recent_entries;
        self.projects = projects;
        self.todays_statistic = todays_statistic;
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.now = Timezone::now();
        self.render_main_view(frame, area);
        match &mut self.timer_mode {
            TimerMode::StartNewEntry(state)
                if matches!(state.mode, StartOrEditEntryState::ProjectSelection) =>
            {
                render_project_selection_list(
                    frame,
                    area,
                    &self.projects,
                    &mut state.project_list_state,
                )
            }
            TimerMode::EditActiveEntry(state)
                if matches!(state.mode, StartOrEditEntryState::ProjectSelection) =>
            {
                render_project_selection_list(
                    frame,
                    area,
                    &self.projects,
                    &mut state.project_list_state,
                )
            }
            _ => (),
        };
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

    fn render_recent_entries(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::new().padding(Padding::symmetric(1, 1));
        let [title_area, entries_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(block.inner(area));
        frame.render_widget(Paragraph::new("RECENT ENTRIES"), title_area);
        let table_block = Block::new().borders(Borders::TOP);
        frame.render_widget(&table_block, entries_area);
        let table_area = table_block.inner(entries_area);

        let headings = Row::new(
            ["Project", "Task", "Description", "Day", "Time", "Total"]
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
                Cell::from(e.start_time.date_naive().to_string()),
                Cell::from(format!(
                    "{} - {}",
                    format_timestamp(&e.start_time),
                    e.end_time
                        .map_or("Running".to_owned(), |ts| format_timestamp(&ts))
                )),
                Cell::from(format!(
                    "{:>8}",
                    delta_hms(&e.start_time, &e.end_time.unwrap_or(self.now))
                )),
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
            Constraint::Length(12), // start time - end time
            Constraint::Length(18), // start time - end time
            Constraint::Length(10), // duration
        ];
        let table = Table::new(rows, column_widths).header(headings);
        frame.render_widget(table, table_area);
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<PageAction> {
        let action = match &mut self.timer_mode {
            TimerMode::NoRunningEntry => {
                match key_event.code {
                    KeyCode::Char('s') => {
                        self.timer_mode = TimerMode::StartNewEntry(StartNewEntryState::default());
                    }
                    KeyCode::Char('q') => {}
                    _ => (),
                };
                None
            }
            TimerMode::ShowActiveEntry(time_entry) => match key_event.code {
                KeyCode::Char('s') => {
                    let entry_id = time_entry.id;
                    self.timer_mode = TimerMode::NoRunningEntry;
                    Some(TrackerAction::StopTracking { entry_id })
                }
                KeyCode::Char('e') => {
                    self.timer_mode = TimerMode::EditActiveEntry(EditActiveEntryState {
                        active_entry: time_entry.clone(),
                        mode: StartOrEditEntryState::ReviewBeforeStart,
                        menu_list_state: ListState::default().with_selected(Some(0)),
                        project_list_state: ListState::default().with_selected(Some(0)),
                    });
                    None
                }
                _ => None,
            },
            TimerMode::StartNewEntry(state) => {
                let action: Option<TrackerAction> = match &mut state.mode {
                    StartOrEditEntryState::ProjectSelection => {
                        match key_event.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                state.project_list_state.select_previous();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                state.project_list_state.select_next();
                            }
                            KeyCode::Enter => {
                                let idx = state.project_list_state.selected().unwrap();
                                let selected_project = self.projects[idx].clone();
                                state.new_entry.project = Some(selected_project);
                                // TODO: continue with task selection
                                state.mode = StartOrEditEntryState::ReviewBeforeStart;
                            }
                            KeyCode::Esc => state.mode = StartOrEditEntryState::ReviewBeforeStart,
                            _ => (),
                        };
                        None
                    }

                    StartOrEditEntryState::TaskSelection => {
                        if let KeyCode::Enter = key_event.code {
                            state.menu_list_state.select_next();
                            state.mode = StartOrEditEntryState::ReviewBeforeStart;
                        }
                        None
                    }
                    StartOrEditEntryState::EnterDescription => match key_event.code {
                        KeyCode::Char(c) => {
                            state.new_entry.description.push(c);
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Backspace => {
                            state.new_entry.description.pop();
                            None
                        }
                        KeyCode::Enter | KeyCode::Esc => {
                            state.mode = StartOrEditEntryState::ReviewBeforeStart;
                            None
                        }
                        _ => None,
                    },
                    StartOrEditEntryState::ReviewBeforeStart => match key_event.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            state.menu_list_state.select_previous();
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            state.menu_list_state.select_next();
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Char('s') if state.new_entry.project.is_some() => {
                            let start_tracking_action = StartTracking {
                                project_id: state.new_entry.project.as_ref().unwrap().id,
                                description: state.new_entry.description.clone(),
                                task_id: None,
                            };
                            self.timer_mode = TimerMode::NoRunningEntry;
                            // Showing the new timer will be triggered by the app after a database
                            // entry has been created
                            Some(start_tracking_action)
                        }
                        KeyCode::Enter => {
                            state.mode = StartOrEditEntryState::from_usize(
                                state.menu_list_state.selected().unwrap_or(0),
                            )
                            .unwrap();
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            self.timer_mode = TimerMode::NoRunningEntry;
                            Some(TrackerAction::ConsumedKey)
                        }
                        _ => None,
                    },
                };
                action
            }
            TimerMode::EditActiveEntry(state) => {
                // Todo port the logic for submenu: project selection, task selection, description input
                let action: Option<TrackerAction> = match &mut state.mode {
                    StartOrEditEntryState::ProjectSelection => {
                        match key_event.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                state.project_list_state.select_previous();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                state.project_list_state.select_next();
                            }
                            KeyCode::Enter => {
                                let idx = state.project_list_state.selected().unwrap();
                                let selected_project = self.projects[idx].clone();
                                state.active_entry.project_id = selected_project.id;
                                // TODO: continue with task selection
                                state.mode = StartOrEditEntryState::ReviewBeforeStart;
                            }
                            KeyCode::Esc => state.mode = StartOrEditEntryState::ReviewBeforeStart,
                            _ => (),
                        };
                        None
                    }

                    StartOrEditEntryState::TaskSelection => {
                        if let KeyCode::Enter = key_event.code {
                            state.menu_list_state.select_next();
                            state.mode = StartOrEditEntryState::ReviewBeforeStart;
                        }
                        None
                    }
                    StartOrEditEntryState::EnterDescription => match key_event.code {
                        KeyCode::Char(c) => {
                            state.active_entry.description.push(c);
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Backspace => {
                            state.active_entry.description.pop();
                            None
                        }
                        KeyCode::Enter | KeyCode::Esc => {
                            state.mode = StartOrEditEntryState::ReviewBeforeStart;
                            None
                        }
                        _ => None,
                    },
                    StartOrEditEntryState::ReviewBeforeStart => match key_event.code {
                        KeyCode::Char('d') => {
                            let entry = state.active_entry.clone();
                            self.timer_mode = TimerMode::ShowActiveEntry(entry.clone());
                            Some(TrackerAction::UpdateEntry(entry))
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            state.menu_list_state.select_previous();
                            None
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            state.menu_list_state.select_next();
                            None
                        }
                        KeyCode::Enter => {
                            state.mode = StartOrEditEntryState::from_usize(
                                state.menu_list_state.selected().unwrap_or(0),
                            )
                            .unwrap();
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Esc => {
                            let entry = state.active_entry.clone();
                            self.timer_mode = TimerMode::ShowActiveEntry(entry);
                            Some(TrackerAction::ConsumedKey)
                        }
                        _ => None,
                    },
                };
                action
            }
        };
        action.map_or_default(|a| Some(PageAction::Tracker(a)))
    }

    fn render_tracker(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .padding(Padding::horizontal(1));
        let tracker_area = block.inner(area);

        let columns = [Constraint::Length(14), Constraint::Min(1)];
        let [time_area, info_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(3)]).areas(tracker_area);
        let [labels_area, values_area] = Layout::horizontal(columns).areas(info_area);

        match &mut self.timer_mode {
            TimerMode::NoRunningEntry => {
                let block = block.title(" READY ");
                frame.render_widget(block, area);
                render_no_active_tracker(frame, tracker_area);
            }
            TimerMode::ShowActiveEntry(entry) => {
                let block = block.title(" RUNNING ");
                frame.render_widget(block, area);
                let project = self.projects.iter().find(|p| p.id == entry.project_id);
                let project_color = project.map_or(style::Color::Yellow, |p| {
                    style::Color::from_str(&p.color).unwrap_or_default()
                });

                let duration = delta_hms(&entry.start_time, &entry.end_time.unwrap_or(self.now));
                let project_name = project.map_or("None".to_string(), |p| p.name.clone());
                let description = entry.description.clone();

                render_active_tracker_info_value(
                    frame,
                    TrackingInfo {
                        duration,
                        project_name,
                        project_color,
                        task: "".into(),
                        description,
                    },
                    time_area,
                    labels_area,
                    values_area,
                );
            }
            TimerMode::StartNewEntry(state) => {
                let block = block.title(" STARTING TRACKER ");
                frame.render_widget(block, area);

                let labels = List::new(vec![
                    Line::from("Project:"),
                    Line::from("Task:"),
                    Line::from("Description:"),
                ]);
                frame.render_stateful_widget(labels, labels_area, &mut state.menu_list_state);

                let project_name = create_input_list_item(
                    "select project...",
                    state.new_entry.project.as_ref().map(|p| p.name.clone()),
                );
                let task_name = create_input_list_item("select task...", None);
                let desc = format_as_active_text_input(
                    &state.new_entry.description,
                    matches!(state.mode, StartOrEditEntryState::EnterDescription),
                );

                let description = create_input_list_item("add description...", desc.into());
                frame.render_stateful_widget(
                    List::new(vec![project_name, task_name, description])
                        .highlight_style(Style::default().bg(Color::DarkGray))
                        .highlight_symbol(">"),
                    values_area,
                    &mut state.menu_list_state,
                );
            }
            TimerMode::EditActiveEntry(state) => {
                let block = block.title(" STARTING TRACKER ");
                frame.render_widget(block, area);

                let labels = List::new(vec![
                    Line::from("Project:"),
                    Line::from("Task:"),
                    Line::from("Description:"),
                    Line::from("Start Time:"),
                ]);
                frame.render_stateful_widget(labels, labels_area, &mut state.menu_list_state);

                let project = self
                    .projects
                    .iter()
                    .find(|p| p.id == state.active_entry.project_id);
                let project_color = project.map_or(style::Color::Yellow, |p| {
                    style::Color::from_str(&p.color).unwrap_or_default()
                });

                let start_time = format_timestamp(&state.active_entry.start_time);
                let project_name = project.map(|p| p.name.clone());

                let project_name = create_input_list_item("select project...", project_name);
                let task_name = create_input_list_item("select task...", None);
                let desc = format_as_active_text_input(
                    &state.active_entry.description,
                    matches!(state.mode, StartOrEditEntryState::EnterDescription),
                );

                let description = create_input_list_item("add description...", desc.into());
                let start_time = ListItem::from(start_time);
                frame.render_stateful_widget(
                    List::new(vec![project_name, task_name, description, start_time])
                        .highlight_style(Style::default().bg(Color::DarkGray))
                        .highlight_symbol(">"),
                    values_area,
                    &mut state.menu_list_state,
                );
            }
        }
    }

    fn render_todays_stats(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(" TODAY ")
            .padding(Padding::horizontal(1));
        frame.render_widget(&block, area);
        let stats_area = block.inner(area);

        let columns = [Constraint::Length(16), Constraint::Min(1)];
        let [labels_area, values_area] = Layout::horizontal(columns).areas(stats_area);
        let labels = Paragraph::new(vec![
            Line::from("Total time:"),
            Line::from("Sessions:"),
            Line::from("Longest session:"),
        ]);
        frame.render_widget(labels, labels_area);

        let now = self.now;
        let mut today_session_count = self.todays_statistic.entries_count;
        let active_session_time = match &self.timer_mode {
            TimerMode::ShowActiveEntry(active_entry) => {
                today_session_count += 1;
                active_entry.end_time.unwrap_or(now) - active_entry.start_time
            }
            TimerMode::EditActiveEntry(state) => {
                today_session_count += 1;
                state.active_entry.end_time.unwrap_or(now) - state.active_entry.start_time
            }
            _ => chrono::TimeDelta::zero(),
        };

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(format_delta_time_statistic(
                    &(self.todays_statistic.total_time + active_session_time),
                )),
                Line::from(today_session_count.to_string()),
                Line::from(format_delta_time_statistic(&std::cmp::max(
                    self.todays_statistic.longest_session,
                    active_session_time,
                ))),
            ])
            .right_aligned(),
            values_area,
        );
    }

    pub fn get_keybind_hints(&self) -> &str {
        match &self.timer_mode {
            TimerMode::NoRunningEntry => "Navigate: ⇅  | Start: s | Quit: q | Switch View: 1/2",
            TimerMode::ShowActiveEntry(_) => {
                "Navigate: ⇅  | Stop: s | Edit: e | Quit: q | Switch View: 1/2"
            }
            TimerMode::StartNewEntry(state) => match state.mode {
                StartOrEditEntryState::ProjectSelection => {
                    "Navigate: ⇅  | Start: s | Quit: q | Switch View: 1/2"
                }
                StartOrEditEntryState::TaskSelection => {
                    "Navigate: ⇅  | Start: s | Quit: q | Switch View: 1/2"
                }
                StartOrEditEntryState::EnterDescription => "Submit: <enter>",
                ReviewBeforeStart => {
                    "Navigate: ⇅  | Start: s | Edit: e | Quit: q | Switch View: 1/2"
                }
            },
            TimerMode::EditActiveEntry(_) => {
                "Navigate: ⇅  | Start/Stop: s | Edit: e | Quit: q | Switch View: 1/2"
            }
        }
    }
}

fn render_active_tracker_info_value(
    frame: &mut Frame<'_>,
    info: TrackingInfo,
    time_area: Rect,
    labels_area: Rect,
    values_area: Rect,
) {
    frame.render_widget(
        Paragraph::new(info.duration)
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

    let project_name = Line::from(info.project_name).style(info.project_color);
    let task_name = Line::from(info.task);
    let description = Line::from(info.description);
    frame.render_widget(
        Paragraph::new(vec![project_name, task_name, description]),
        values_area,
    );
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

fn render_project_selection_list(
    frame: &mut Frame<'_>,
    area: Rect,
    projects: &[database::Project],
    list_state: &mut ListState,
) {
    let centered_area = area.centered(Constraint::Percentage(50), Constraint::Percentage(70));
    frame.render_widget(Clear, centered_area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .padding(Padding::symmetric(1, 1))
        .title("Select Project");
    frame.render_widget(&block, centered_area);

    let project_list = List::new(projects.iter().map(|p| {
        ListItem::new(p.name.as_str())
            .style(Style::new().fg(style::Color::from_str(&p.color).unwrap()))
    }))
    .highlight_style(Style::default().bg(Color::DarkGray))
    .highlight_symbol("> ");
    frame.render_stateful_widget(project_list, block.inner(centered_area), list_state);
}
