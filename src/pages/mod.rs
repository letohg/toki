use ratatui::{Frame, layout::Rect};

use crate::{
    database::{Timestamp, Timezone},
    pages::tracker::TrackerAction,
};

pub mod projects;
pub mod tracker;

pub enum PageAction {
    Tracker(TrackerAction),
    Projects,
}

pub fn delta_hms(start: &Timestamp, end: &Option<Timestamp>) -> String {
    let delta: chrono::TimeDelta = end.unwrap_or(Timezone::now()) - start;
    let h = delta.num_hours();
    let m = delta.num_minutes() % 60;
    let s = delta.num_seconds() % 60;
    if h > 0 {
        return format!("{:02}:{:02}:{:02}", h, m, s);
    }
    format!("{:02}:{:02}", m, s)
}

pub fn format_timestamp(ts: &Timestamp) -> String {
    ts.format("%H:%M").to_string()
}
