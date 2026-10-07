use chrono::Datelike;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{self, Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, TableState, Wrap,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Cell, Paragraph, Row, Table},
};
use std::str::FromStr;

use crate::database::{self, EntityId, TimeEntry, Timestamp, Timezone};
use crate::pages::tracker::TrackerAction::StartTracking;
use crate::pages::{
    PageAction, format_duration_clock, format_duration_clock_hm, format_duration_hms_verbose,
    format_timestamp,
};
use crate::pages::{delta_hms, format_duration_hm_verbose};
use crate::{notify, theme};

#[derive(Clone, FromPrimitive)]
enum StartOrEditEntryState {
    ProjectSelection,
    TaskSelection,
    EnterDescription,
    EditStartTime,
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
    time_edit_input: String,
}
impl EditActiveEntryState {
    fn default() -> Self {
        Self {
            active_entry: TimeEntry::default(),
            mode: StartOrEditEntryState::ReviewBeforeStart,
            menu_list_state: ListState::default().with_selected(Some(0)),
            project_list_state: ListState::default().with_selected(Some(0)),
            time_edit_input: String::default(),
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
    metrics: database::Metrics,
    recent_entries_table_state: TableState,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            now: Timezone::now(),
            recent_entries: Vec::new(),
            projects: Vec::new(),
            timer_mode: TimerMode::NoRunningEntry,
            metrics: database::Metrics::default(),
            recent_entries_table_state: TableState::default().with_selected(None),
        }
    }

    pub fn refresh(
        &mut self,
        active_entry: Option<database::TimeEntry>,
        recent_entries: Vec<database::TimeEntry>,

        projects: Vec<database::Project>,
        metrics: database::Metrics,
    ) {
        if let Some(e) = active_entry {
            self.timer_mode = TimerMode::ShowActiveEntry(e);
        }

        self.recent_entries = recent_entries;
        self.projects = projects;
        self.metrics = metrics;
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
            Layout::vertical([Constraint::Length(8), Constraint::Min(3)]).areas(area);

        let metrics_max_width = ("in 10.000 sessions".len() + 4) as u16;
        let [tracker_area, dm, wt] = Layout::horizontal([
            Constraint::Min(40),
            // Constraint::Length(1),
            Constraint::Max(metrics_max_width),
            Constraint::Max(metrics_max_width),
        ])
        .areas(tracker_stats_area);
        let split = [Constraint::Percentage(50), Constraint::Percentage(50)];
        let [d, m] = Layout::vertical(split).areas(dm);
        let [w, t] = Layout::vertical(split).areas(wt);

        self.render_tracker(frame, tracker_area);
        self.render_today_metrics(frame, d, "Today", &self.metrics.today_metrics);
        self.render_metrics(frame, w, "This Week", &self.metrics.week_metrics);
        self.render_metrics(frame, m, "This Month", &self.metrics.month_metrics);
        self.render_metrics(frame, t, "Total", &self.metrics.total_metrics);
        // self.render_todays_stats(frame, today_stats_area);
        self.render_recent_entries(frame, entries_area);
    }

    fn render_recent_entries(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::new(); //.padding(Padding::symmetric(1, 1));
        let [title_area, entries_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(block.inner(area));
        let today = Timezone::now();
        frame.render_widget(
            Line::from(vec![
                Span::from(" RECENT ENTRIES").bold().fg(theme::MAUVE),
                Span::from(format!(" · {}", today.format("%A, %d %B %Y"))),
            ]),
            title_area,
        );
        let table_block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::OVERLAY_0);
        frame.render_widget(&table_block, entries_area);
        let table_area = table_block.inner(entries_area);

        let headings = Row::new(
            ["Name", "Task", "Description", " Day", "Time", "Total"]
                .into_iter()
                .map(|t| Cell::from(t).bold().fg(theme::ROSEWATER)),
        )
        .underlined();
        let rows = self.recent_entries.iter().enumerate().map(|(i, e)| {
            let project = self.projects.iter().find(|p| p.id == e.project_id);
            let project_color = project.map_or(style::Color::Yellow, |p| {
                style::Color::from_str(&p.color).unwrap_or_default()
            });
            let row_fg_color = e.end_time.map_or(theme::GREEN, |_| theme::SUBTEXT_1);
            let row_bg_color = if i % 2 == 0 {
                theme::BASE
            } else {
                theme::SURFACE_0
            };
            Row::new(vec![
                Cell::from(project.map_or("None".to_string(), |p| format!("• {}", p.name)))
                    .fg(project_color),
                "-".into(),
                Cell::from(if e.description.is_empty() {
                    "-".to_string()
                } else if e.description.len() >= 18 {
                    format!("{}..", &e.description[..18])
                } else {
                    e.description.clone()
                })
                .fg(row_fg_color),
                Cell::from(e.start_time.format(" %d-%b").to_string()).fg(row_fg_color),
                Cell::from(format!(
                    "{} - {}",
                    format_timestamp(&e.start_time),
                    e.end_time
                        .map_or("now".to_owned(), |ts| format_timestamp(&ts))
                ))
                .fg(row_fg_color),
                Cell::from(format!(
                    "{:>8}",
                    delta_hms(&e.start_time, &e.end_time.unwrap_or(self.now))
                ))
                .fg(row_fg_color),
            ])
            // TODO: better coloring
            .style(style::Style::default().bg(row_bg_color))
        });
        let column_widths = vec![
            Constraint::Length(15),
            Constraint::Min(0),
            Constraint::Fill(20),
            Constraint::Length(8),  // start time - end time
            Constraint::Length(14), // start time - end time
            Constraint::Length(8),  // duration
        ];
        let table = Table::new(rows, column_widths)
            .header(headings)
            .row_highlight_style(style::Style::new().fg(theme::BASE).bg(theme::PINK))
            .cell_highlight_style(style::Style::new().fg(theme::BASE).bg(theme::LAVENDER));
        frame.render_stateful_widget(table, table_area, &mut self.recent_entries_table_state);
    }

    pub fn handle_key_event(
        &mut self,
        key_event: KeyEvent,
        notifier: &mut notify::Notifier,
    ) -> Option<PageAction> {
        let action = match &mut self.timer_mode {
            TimerMode::NoRunningEntry => match key_event.code {
                KeyCode::Char('r') => {
                    let row = self.recent_entries_table_state.selected().unwrap();
                    self.recent_entries_table_state =
                        self.recent_entries_table_state.with_selected_cell(None);
                    let entry_to_restart = &self.recent_entries[row];
                    notifier.notify_info("New Tracker started");
                    Some(TrackerAction::StartTracking {
                        project_id: entry_to_restart.project_id,
                        description: entry_to_restart.description.clone(),
                        task_id: entry_to_restart.task_id,
                    })
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.recent_entries_table_state.selected().is_none() {
                        self.recent_entries_table_state =
                            self.recent_entries_table_state.with_selected(0);
                    } else {
                        self.recent_entries_table_state.select_previous();
                    }
                    Some(TrackerAction::ConsumedKey)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.recent_entries_table_state.selected().is_none() {
                        self.recent_entries_table_state =
                            self.recent_entries_table_state.with_selected(0);
                    } else {
                        self.recent_entries_table_state.select_next();
                    }
                    Some(TrackerAction::ConsumedKey)
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    if self.recent_entries_table_state.selected().is_none() {
                        self.recent_entries_table_state =
                            self.recent_entries_table_state.with_selected(0);
                    } else {
                        self.recent_entries_table_state.select_previous_column();
                    }
                    Some(TrackerAction::ConsumedKey)
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if self.recent_entries_table_state.selected_column().is_none() {
                        self.recent_entries_table_state =
                            self.recent_entries_table_state.with_selected_column(0);
                    } else {
                        self.recent_entries_table_state.select_next_column();
                    }
                    Some(TrackerAction::ConsumedKey)
                }
                KeyCode::Char('s') => {
                    self.timer_mode = TimerMode::StartNewEntry(StartNewEntryState::default());
                    Some(TrackerAction::ConsumedKey)
                }
                _ => None,
            },
            TimerMode::ShowActiveEntry(time_entry) => match key_event.code {
                KeyCode::Esc => {
                    self.recent_entries_table_state =
                        self.recent_entries_table_state.with_selected_cell(None);
                    Some(TrackerAction::ConsumedKey)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.recent_entries_table_state.selected().is_none() {
                        self.recent_entries_table_state =
                            self.recent_entries_table_state.with_selected(0);
                    } else {
                        self.recent_entries_table_state.select_previous();
                    }
                    Some(TrackerAction::ConsumedKey)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.recent_entries_table_state.selected().is_none() {
                        self.recent_entries_table_state =
                            self.recent_entries_table_state.with_selected(0);
                    } else {
                        self.recent_entries_table_state.select_next();
                    }
                    Some(TrackerAction::ConsumedKey)
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    if self.recent_entries_table_state.selected().is_none() {
                        self.recent_entries_table_state =
                            self.recent_entries_table_state.with_selected(0);
                    } else {
                        self.recent_entries_table_state.select_previous_column();
                    }
                    Some(TrackerAction::ConsumedKey)
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if self.recent_entries_table_state.selected_column().is_none() {
                        self.recent_entries_table_state =
                            self.recent_entries_table_state.with_selected_column(0);
                    } else {
                        self.recent_entries_table_state.select_next_column();
                    }
                    Some(TrackerAction::ConsumedKey)
                }
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
                        time_edit_input: format_timestamp(&time_entry.start_time),
                    });
                    Some(TrackerAction::ConsumedKey)
                }
                _ => None,
            },
            TimerMode::StartNewEntry(state) => {
                let action: Option<TrackerAction> = match &mut state.mode {
                    StartOrEditEntryState::ProjectSelection => {
                        match key_event.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                state.project_list_state.select_previous();
                                Some(TrackerAction::ConsumedKey)
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                state.project_list_state.select_next();
                                Some(TrackerAction::ConsumedKey)
                            }
                            KeyCode::Enter => {
                                let idx = state.project_list_state.selected().unwrap();
                                let selected_project = self.projects[idx].clone();
                                state.new_entry.project = Some(selected_project);
                                // TODO: continue with task selection
                                state.mode = StartOrEditEntryState::ReviewBeforeStart;
                                Some(TrackerAction::ConsumedKey)
                            }
                            KeyCode::Esc => {
                                state.mode = StartOrEditEntryState::ReviewBeforeStart;
                                Some(TrackerAction::ConsumedKey)
                            }
                            _ => None,
                        }
                    }

                    StartOrEditEntryState::TaskSelection => match key_event.code {
                        KeyCode::Enter => {
                            state.menu_list_state.select_next();
                            state.mode = StartOrEditEntryState::ReviewBeforeStart;
                            Some(TrackerAction::ConsumedKey)
                        }
                        _ => None,
                    },
                    StartOrEditEntryState::EnterDescription => match key_event.code {
                        KeyCode::Char(c) => {
                            state.new_entry.description.push(c);
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Backspace => {
                            state.new_entry.description.pop();
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Enter | KeyCode::Esc => {
                            state.mode = StartOrEditEntryState::ReviewBeforeStart;
                            Some(TrackerAction::ConsumedKey)
                        }
                        _ => None,
                    },

                    StartOrEditEntryState::EditStartTime => match key_event.code {
                        KeyCode::Char(c) if c.is_ascii_digit() || c == ':' => {
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Backspace => None,
                        KeyCode::Enter => None,
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
                        KeyCode::Char(' ') if state.new_entry.project.is_some() => {
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
                        KeyCode::Char(' ') => {
                            notifier
                                .notify_info("Cannot start tracker without selecting a project");
                            Some(TrackerAction::ConsumedKey)
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
                                Some(TrackerAction::ConsumedKey)
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                state.project_list_state.select_next();
                                Some(TrackerAction::ConsumedKey)
                            }
                            KeyCode::Enter => {
                                let idx = state.project_list_state.selected().unwrap();
                                let selected_project = self.projects[idx].clone();
                                state.active_entry.project_id = selected_project.id;
                                // TODO: continue with task selection
                                state.mode = StartOrEditEntryState::ReviewBeforeStart;
                                Some(TrackerAction::ConsumedKey)
                            }
                            KeyCode::Esc => {
                                state.mode = StartOrEditEntryState::ReviewBeforeStart;
                                Some(TrackerAction::ConsumedKey)
                            }
                            _ => None,
                        }
                    }

                    StartOrEditEntryState::TaskSelection => match key_event.code {
                        KeyCode::Enter => {
                            state.menu_list_state.select_next();
                            state.mode = StartOrEditEntryState::ReviewBeforeStart;
                            Some(TrackerAction::ConsumedKey)
                        }
                        _ => None,
                    },
                    StartOrEditEntryState::EnterDescription => match key_event.code {
                        KeyCode::Char(c) => {
                            state.active_entry.description.push(c);
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Backspace => {
                            state.active_entry.description.pop();
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Enter | KeyCode::Esc => {
                            state.mode = StartOrEditEntryState::ReviewBeforeStart;
                            Some(TrackerAction::ConsumedKey)
                        }
                        _ => None,
                    },
                    StartOrEditEntryState::EditStartTime => match key_event.code {
                        KeyCode::Esc => {
                            state.mode = StartOrEditEntryState::ReviewBeforeStart;
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Backspace => {
                            state.time_edit_input.pop();
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() || c == ':' => {
                            state.time_edit_input.push(c);
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Enter => {
                            let time = chrono::NaiveTime::parse_from_str(
                                &format!("{:0>4}", &state.time_edit_input),
                                "%H%M",
                            )
                            .or_else(|_| {
                                chrono::NaiveTime::parse_from_str(&state.time_edit_input, "%H:%M")
                            });
                            match time {
                                Ok(t) => {
                                    state.active_entry.start_time =
                                        state.active_entry.start_time.with_time(t).single().unwrap()
                                }
                                Err(e) => (),
                            }

                            state.mode = StartOrEditEntryState::ReviewBeforeStart;
                            Some(TrackerAction::ConsumedKey)
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
                            Some(TrackerAction::ConsumedKey)
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            state.menu_list_state.select_next();
                            Some(TrackerAction::ConsumedKey)
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
            Layout::vertical([Constraint::Length(1), Constraint::Length(6)]).areas(tracker_area);
        let [labels_area, values_area] = Layout::horizontal(columns).areas(info_area);

        match &mut self.timer_mode {
            TimerMode::NoRunningEntry => {
                let block = block
                    .title(" READY ")
                    .title_style(theme::GREEN)
                    .border_style(theme::GREEN);
                frame.render_widget(block, area);
                render_no_active_tracker(frame, tracker_area);
            }
            TimerMode::ShowActiveEntry(entry) => {
                let block = block
                    .title(" RUNNING ")
                    .title_style(theme::MAROON)
                    .border_style(theme::MAROON);
                frame.render_widget(block, area);
                let project = self.projects.iter().find(|p| p.id == entry.project_id);
                let project_color = project.map_or(style::Color::Yellow, |p| {
                    style::Color::from_str(&p.color).unwrap_or_default()
                });

                let duration = delta_hms(&entry.start_time, &entry.end_time.unwrap_or(self.now));
                let project_name = project.map_or("None".to_string(), |p| p.name.clone());
                frame.render_widget(
                    Paragraph::new(duration)
                        .style(style::Color::Green)
                        .centered()
                        .bold(),
                    time_area,
                );

                let labels = Paragraph::new(vec![
                    Line::from("Project     :"),
                    Line::from("Task        :"),
                    Line::from("Description :"),
                ]);
                frame.render_widget(labels, labels_area);

                let project_name = Line::from(project_name).style(project_color);
                let task_name = Line::from("Default").fg(theme::OVERLAY_0);
                let description = Line::from(entry.description.clone());
                frame.render_widget(
                    Paragraph::new(vec![project_name, task_name, description])
                        .wrap(Wrap { trim: false }),
                    values_area,
                );
            }
            TimerMode::StartNewEntry(state) => {
                let block = block
                    .title(" STARTING TRACKER ")
                    .title_style(theme::YELLOW)
                    .border_style(theme::YELLOW);
                frame.render_widget(block, area);

                let labels = List::new(vec![
                    Line::from("Project     :"),
                    Line::from("Task        :"),
                    Line::from("Description :"),
                ]);
                frame.render_stateful_widget(labels, labels_area, &mut state.menu_list_state);

                let project_name = create_input_list_item(
                    "select project...",
                    state.new_entry.project.as_ref().map(|p| p.name.clone()),
                );
                let task_name = create_input_list_item("Default", None);
                let desc = format_as_active_text_input(
                    &state.new_entry.description,
                    matches!(state.mode, StartOrEditEntryState::EnterDescription),
                );

                let input_field_width = (values_area.width - 3) as usize;

                let description = if desc.len() > input_field_width {
                    let trailing_len = desc.len() - input_field_width;
                    create_input_list_item(
                        "add description...",
                        Some(format!("..{}", &desc[trailing_len..])),
                    )
                } else {
                    create_input_list_item("add description...", desc.into())
                };
                frame.render_stateful_widget(
                    List::new(vec![project_name, task_name, description])
                        // .highlight_style(Style::default().bg(theme::SURFACE_0))
                        .highlight_style(Style::default().fg(theme::LAVENDER).bold())
                        .highlight_symbol("› "),
                    values_area,
                    &mut state.menu_list_state,
                );
            }
            TimerMode::EditActiveEntry(state) => {
                let block = block
                    .title(" EDITING TRACKER ")
                    .title_style(theme::YELLOW)
                    .border_style(theme::YELLOW);
                frame.render_widget(block, area);

                let labels = List::new(vec![
                    Line::from("Project     :"),
                    Line::from("Task        :"),
                    Line::from("Description :"),
                    Line::from("Start Time  :"),
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
                let task_name = create_input_list_item("Default", None);
                let desc = format_as_active_text_input(
                    &state.active_entry.description,
                    matches!(state.mode, StartOrEditEntryState::EnterDescription),
                );

                let start_time = format_as_active_text_input(
                    &state.time_edit_input,
                    matches!(state.mode, StartOrEditEntryState::EditStartTime),
                );

                let description = create_input_list_item("add description...", desc.into());
                let start_time = ListItem::from(start_time);
                frame.render_stateful_widget(
                    List::new(vec![project_name, task_name, description, start_time])
                        // .highlight_style(Style::default().bg(theme::SURFACE_0))
                        .highlight_style(Style::default().fg(theme::LAVENDER).bold())
                        .highlight_symbol("› "),
                    values_area,
                    &mut state.menu_list_state,
                );
            }
        }
    }

    fn render_today_metrics(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        title: &str,
        metrics: &database::IntervalMetrics,
    ) {
        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(theme::LAVENDER)
            .title(format!(" {} ", title))
            .padding(Padding::horizontal(1));
        frame.render_widget(&block, area);
        let area = block.inner(area);
        let delta = metrics.total_time - chrono::Duration::hours(8);
        let delta_style = if delta > chrono::Duration::zero() {
            style::Style::new().italic().fg(theme::GREEN)
        } else {
            style::Style::new().italic().fg(theme::MAROON)
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(format_duration_hms_verbose(&metrics.total_time)),
                Line::from(format_duration_clock_hm(&delta)).style(delta_style),
            ])
            .right_aligned(),
            area,
        );
    }

    fn render_metrics(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        title: &str,
        metrics: &database::IntervalMetrics,
    ) {
        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(theme::LAVENDER)
            .title(format!(" {} ", title))
            .padding(Padding::horizontal(1));
        frame.render_widget(&block, area);
        let area = block.inner(area);

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(format_duration_hms_verbose(&metrics.total_time)),
                Line::from(format!("in {} sessions", metrics.session_count))
                    .italic()
                    .fg(theme::OVERLAY_0),
            ])
            .right_aligned(),
            area,
        );
    }

    fn render_todays_stats(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(theme::LAVENDER)
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
        let mut today_session_count = self.metrics.today.entries_count;
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
                Line::from(format_duration_hms_verbose(
                    &(self.metrics.today.total_time + active_session_time),
                )),
                Line::from(today_session_count.to_string()),
                Line::from(format_duration_hms_verbose(&std::cmp::max(
                    self.metrics.today.longest_session,
                    active_session_time,
                ))),
            ])
            .right_aligned(),
            values_area,
        );
    }

    pub fn get_keybind_hints(&self) -> &str {
        // TODO:
        // vec![
        //     ("Start", 's')
        //     ("Quit", 'q')
        // ];
        match &self.timer_mode {
            TimerMode::NoRunningEntry => "Select: ⇅  | Start: s | Quit: q",
            TimerMode::ShowActiveEntry(_) => "Select: ⇅  | Stop: s | Edit: e | Quit: q",
            TimerMode::StartNewEntry(state) => match state.mode {
                StartOrEditEntryState::ProjectSelection => {
                    "Select: ⇅  | Choose: <enter> | Cancle <esc> | Quit: q"
                }
                StartOrEditEntryState::TaskSelection => "Select: ⇅  | Quit: q",
                StartOrEditEntryState::EnterDescription => "Submit: <enter>",
                StartOrEditEntryState::ReviewBeforeStart => {
                    "Select: ⇅  | Start: <space> | Canlce: <esc> | Quit: q"
                }
                StartOrEditEntryState::EditStartTime => {
                    "Select: ⇅  | Start: <space> | Canlce: <esc> | Quit: q"
                }
            },
            TimerMode::EditActiveEntry(_) => "Select: ⇅  | Done: d | Cancle: <esc> | Quit: q",
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
        // "[ ".into(),
        match content {
            None => Span::raw(placeholder).italic().fg(theme::OVERLAY_0),
            Some(c) if c.is_empty() => Span::raw(placeholder).italic().fg(theme::OVERLAY_0),
            Some(c) => Span::raw(c),
        },
        // Span::raw(" ]"),
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
        .bg(theme::BASE)
        .padding(Padding::symmetric(1, 1))
        .title("Select Project");
    frame.render_widget(&block, centered_area);

    let project_list = List::new(projects.iter().map(|p| {
        ListItem::new(p.name.as_str())
            .style(Style::new().fg(style::Color::from_str(&p.color).unwrap()))
    }))
    .highlight_style(Style::default().bg(theme::SURFACE_0))
    .highlight_symbol("› ");
    frame.render_stateful_widget(project_list, block.inner(centered_area), list_state);
}
