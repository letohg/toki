use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{self, Color};
use ratatui::text::{Line, Span};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Cell, Paragraph, Row, Table},
};
use std::str::FromStr;

use crate::database::{self, EntityId};
use crate::views::delta_hms;
use crate::views::format_timestamp;

pub struct Timer {
    active_entry: Option<database::TimeEntry>,
    recent_entries: Vec<database::TimeEntry>,
    projects: Vec<database::Project>,
}

pub enum TimerAction {
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

impl Timer {
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
        let [tracker_area, entries_area] =
            Layout::vertical([Constraint::Length(5), Constraint::Min(0)]).areas(area);

        let tracking_info: TrackingInfo = match &self.active_entry {
            Some(entry) => {
                let project = self.projects.iter().find(|p| p.id == entry.project_id);
                let project_color = project.map_or(style::Color::Yellow, |p| {
                    style::Color::from_str(&p.color).unwrap_or_default()
                });

                let duration = delta_hms(&entry.start_time, &entry.end_time);
                let project_name = project.map_or("None".to_string(), |p| p.name.clone());
                let description = entry.description.clone();
                TrackingInfo {
                    duration,
                    project_name,
                    project_color,
                    task: "TODO".to_owned(),
                    description,
                }
            }
            None => TrackingInfo {
                duration: "--:--:--".to_owned(),
                project_name: "[....]".to_owned(),
                project_color: style::Color::White,
                task: "[....]".to_owned(),
                description: "[....]".to_owned(),
            },
        };
        let status = Span::styled(
            match tracking_info.duration == "--:--:--" {
                true => "Not Tracking",
                false => "TRACKING",
            },
            style::Color::Cyan,
        )
        .into_centered_line();
        let duration =
            Span::styled(tracking_info.duration, style::Color::Cyan).into_centered_line();
        let info = vec![
            Row::new(vec![
                Cell::from("Project"),
                Cell::from(tracking_info.project_name),
            ])
            .style(style::Style::new().fg(tracking_info.project_color)),
            Row::new(vec![Cell::from("Task"), Cell::from(tracking_info.task)]),
            Row::new(vec![
                Cell::from("Description:"),
                Cell::from(tracking_info.description),
            ]),
        ];
        let [a1, a2, a3] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .areas(tracker_area);
        let table = Table::new(info, vec![Constraint::Length(13), Constraint::Min(7)]);

        frame.render_widget(duration, a1);
        frame.render_widget(status, a2);
        frame.render_widget(table, a3);

        let rows = self.recent_entries.iter().map(|e| {
            let project = self.projects.iter().find(|p| p.id == e.project_id);
            let project_color = project.map_or(style::Color::Yellow, |p| {
                style::Color::from_str(&p.color).unwrap()
            });
            Row::new(vec![
                Cell::from("*").style(style::Style::new().fg(project_color)),
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

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<TimerAction> {
        match key_event.code {
            KeyCode::Char('s') if let Some(entry) = &self.active_entry => {
                Some(TimerAction::StopTracking(entry.id))
            }
            KeyCode::Char('s') => {
                let project_id = 1;
                let description = "Description".to_owned();
                let task_id = None;
                Some(TimerAction::StartTracking(project_id, description, task_id))
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
}
