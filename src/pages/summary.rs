use chrono::Datelike;
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::layout::Layout;
use ratatui::style::{self, Stylize};
use ratatui::widgets::{self, Bar, Block, Borders, Cell, Padding, Paragraph, Row, Table};
use ratatui::{Frame, layout::Rect};
use ratatui::{layout::Constraint, style::Style};
use std::collections::HashMap;
use std::str::FromStr;

use crate::database::{self, Timezone};

pub struct Summary {
    entries: Vec<database::TimeEntry>,
    projects: Vec<database::Project>,
}

impl Summary {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            projects: Vec::new(),
        }
    }

    pub fn refresh(&mut self, entries: Vec<database::TimeEntry>, projects: Vec<database::Project>) {
        self.entries = entries;
        self.projects = projects;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new().padding(Padding::symmetric(1, 1));
        let [title_area, entries_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(block.inner(area));
        frame.render_widget(Paragraph::new("Projects"), title_area);
        let table_block = Block::new().borders(Borders::TOP);
        frame.render_widget(&table_block, entries_area);
        let table_area = table_block.inner(entries_area);

        let this_month = Timezone::now().month();
        let this_month_entries = self
            .entries
            .iter()
            .filter(|e| e.start_time.month() == this_month);

        let time_per_day = this_month_entries
            .into_group_map_by(|e| e.start_time.day())
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    v.iter()
                        .map(|e| e.end_time.unwrap_or(Timezone::now()) - e.start_time)
                        .sum(),
                )
            })
            .collect::<HashMap<u32, chrono::TimeDelta>>();
        let bars = time_per_day
            .iter()
            .sorted_by(|(d1, t1), (d2, t2)| Ord::cmp(t1, t2))
            .map(|(d, t)| widgets::Bar::with_label(d.to_string(), t.num_hours() as u64))
            .collect::<Vec<widgets::Bar<'_>>>();

        let summary_chart = ratatui::widgets::BarChart::new(bars)
            .block(Block::bordered().title("BarChart"))
            .bar_width(5)
            .bar_gap(2)
            .label_style(Style::new().white());
        frame.render_widget(summary_chart, area);
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> bool {
        false
    }
}
