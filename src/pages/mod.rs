use ratatui::{Frame, layout::Rect};

use crate::{
    database::{Timestamp, Timezone},
    pages::tracker::TrackerAction,
};

pub mod projects;
pub mod summary;
pub mod tracker;

pub enum PageAction {
    Tracker(TrackerAction),
    Projects,
}

pub fn delta_hms(start: &Timestamp, end: &Timestamp) -> String {
    let delta: chrono::TimeDelta = *end - *start;
    format_duration_clock(delta)
}

pub fn format_duration_clock(delta: chrono::TimeDelta) -> String {
    let h = delta.num_hours();
    let m = delta.num_minutes() % 60;
    let s = delta.num_seconds() % 60;
    if h > 0 {
        return format!("{:02}:{:02}:{:02}", h, m, s);
    }
    format!("{:02}:{:02}", m, s)
}

pub fn format_duration_verbose(delta: &chrono::TimeDelta) -> String {
    let h = delta.num_hours();
    let m = delta.num_minutes() % 60;
    let s = delta.num_seconds() % 60;
    if h > 0 {
        return format!("{:02}h {:02}m {:02}s", h, m, s);
    }
    format!("{:02}m {:02}s", m, s)
}

pub fn format_timestamp(ts: &Timestamp) -> String {
    ts.format("%H:%M").to_string()
}
