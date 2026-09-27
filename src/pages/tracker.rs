use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{self, Color, Stylize};
use ratatui::text::{Line, Span, ToLine};
use ratatui::widgets::{Block, Wrap};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Cell, Paragraph, Row, Table},
};
use std::str::FromStr;

use crate::database::{self, EntityId};
use crate::pages::delta_hms;
use crate::pages::format_timestamp;

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
        let [tracker_area, entries_area] =
            Layout::vertical([Constraint::Length(6), Constraint::Min(3)]).areas(area);

        let tracker_block = Block::bordered().title("Tracker");
        let recent_entries_block = Block::new().title("RECENT ENTRIES");
        frame.render_widget(&tracker_block, tracker_area);
        frame.render_widget(&recent_entries_block, entries_area);
        self.render_tracker(frame, tracker_block.inner(tracker_area));
        self.render_recent_entries(frame, recent_entries_block.inner(entries_area));
    }

    fn render_recent_entries(&self, frame: &mut Frame<'_>, entries_area: Rect) {
        let headings = Row::new(
            ["Project", "Task", "Description", "Time", "Total"]
                .into_iter()
                .map(|t| Cell::from(t).bold()),
        );
        let rows = self.recent_entries.iter().map(|e| {
            let project = self.projects.iter().find(|p| p.id == e.project_id);
            let project_color = project.map_or(style::Color::Yellow, |p| {
                style::Color::from_str(&p.color).unwrap_or_default()
            });
            Row::new(vec![
                Cell::from(project.map_or("None".to_string(), |p| p.name.clone()))
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
        });
        let column_widths = vec![
            Constraint::Length(15),
            Constraint::Min(0),
            Constraint::Min(0),
            Constraint::Length(16), // start time - end time
            Constraint::Length(8),  // duration
        ];
        let table = Table::new(rows, column_widths).header(headings);
        frame.render_widget(table, entries_area);
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

    fn render_tracker(&self, frame: &mut Frame<'_>, tracker_area: Rect) {
        let info = self.get_active_entry_info();
        let columns = [Constraint::Length(14), Constraint::Min(1)];
        let [label_area, values_area] = Layout::horizontal(columns).areas(tracker_area);

        let status = match info {
            Some(_) => Span::styled("TRACKING", style::Color::Green),
            None => Span::styled("NOT TRACKING", style::Color::LightRed),
        };
        frame.render_widget(
            Paragraph::new(vec![
                status.to_line(),
                Line::from("Project:"),
                Line::from("Task:"),
                Line::from("Description:"),
            ])
            .wrap(Wrap { trim: true }),
            label_area,
        );

        let duration = Line::styled(
            info.as_ref().map_or_default(|i| i.duration.clone()),
            style::Color::Green,
        );
        let project_name = Line::from(info.as_ref().map_or_default(|i| i.project_name.clone()))
            .style(info.as_ref().map_or_default(|i| i.project_color));
        let task_name = Line::from(info.as_ref().map_or_default(|i| i.task.clone()));
        let description = Line::from(info.as_ref().map_or_default(|i| i.description.clone()));
        frame.render_widget(
            Paragraph::new(vec![duration, project_name, task_name, description]),
            values_area,
        );
    }
}
