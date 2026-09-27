use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{self, Color, Stylize};
use ratatui::text::{Line, Span, ToLine};
use ratatui::widgets::{Block, Borders, Padding, Wrap};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Cell, Paragraph, Row, Table},
};
use std::str::FromStr;

use crate::database::{self, EntityId};
use crate::pages::format_timestamp;
use crate::pages::{delta_hms, tracker};

pub struct Tracker {
    active_entry: Option<database::TimeEntry>,
    recent_entries: Vec<database::TimeEntry>,
    projects: Vec<database::Project>,
}

pub enum TrackerAction {
    StartTracking(EntityId, String, Option<EntityId>),
    StopTracking(EntityId),
}

struct TrackingInfo {
    duration: String,
    project_name: String,
    project_color: Color,
    task: String,
    description: String,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            active_entry: None,
            recent_entries: Vec::new(),
            projects: Vec::new(),
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
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
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

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<TrackerAction> {
        match key_event.code {
            KeyCode::Char('s') if let Some(entry) = &self.active_entry => {
                Some(TrackerAction::StopTracking(entry.id))
            }
            KeyCode::Char('s') => {
                let project_id = 1;
                let description = "Description".to_owned();
                let task_id = None;
                Some(TrackerAction::StartTracking(
                    project_id,
                    description,
                    task_id,
                ))
            }
            KeyCode::Char('e') => {
                // open edit entry mode
                // Project:     [...]
                // Task:        [...]
                // Description: [...]
                None
            }
            _ => None,
        }
    }

    fn render_tracker(&self, frame: &mut Frame<'_>, area: Rect) {
        let info = self.get_active_entry_info();

        let block = Block::bordered()
            .title(if info.is_some() {
                " TRACKING "
            } else {
                " READY "
            })
            .border_type(ratatui::widgets::BorderType::Rounded)
            .padding(Padding::horizontal(1));
        let tracker_area = block.inner(area);

        if info.is_none() {
            frame.render_widget(&block, area);
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
            return;
        }

        frame.render_widget(&block, area);

        let columns = [Constraint::Length(14), Constraint::Min(1)];
        let [time_area, info_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(3)]).areas(tracker_area);
        let [label_area, values_area] = Layout::horizontal(columns).areas(info_area);

        frame.render_widget(
            Paragraph::new(info.as_ref().map_or_default(|i| i.duration.clone()))
                .style(style::Color::Green)
                .centered(),
            time_area,
        );

        frame.render_widget(
            Paragraph::new(vec![
                Line::from("Project:"),
                Line::from("Task:"),
                Line::from("Description:"),
            ])
            .wrap(Wrap { trim: true }),
            label_area,
        );

        let project_name = Line::from(info.as_ref().map_or_default(|i| i.project_name.clone()))
            .style(info.as_ref().map_or_default(|i| i.project_color));
        let task_name = Line::from(info.as_ref().map_or_default(|i| i.task.clone()));
        let description = Line::from(info.as_ref().map_or_default(|i| i.description.clone()));
        frame.render_widget(
            Paragraph::new(vec![project_name, task_name, description]),
            values_area,
        );
    }

    fn render_todays_stats(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(" TODAY ")
            .padding(Padding::horizontal(1));
        frame.render_widget(&block, area);
        let stats_area = block.inner(area);
    }
}
