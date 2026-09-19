use chrono::Timelike;
use resolve_path::PathResolveExt;
use rusqlite::OptionalExtension;

type EntityId = i64;

fn get_now_utc_py() -> String {
    let now = chrono::Utc::now();
    if now.nanosecond() == 0 {
        now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
    } else {
        now.to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
    }
}

pub struct TimeEntry {
    pub id: EntityId,
    pub project_id: EntityId,
    pub task_id: Option<EntityId>,
    pub description: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub created_at: String,
}

impl TimeEntry {
    pub fn new(project_id: EntityId, description: String, task_id: Option<EntityId>) -> Self {
        let now = get_now_utc_py();
        TimeEntry {
            id: 0, // This should be set by the database when inserting
            project_id,
            task_id,
            description,
            start_time: now.clone(),
            end_time: None,
            created_at: now,
        }
    }
}

pub struct Database {}

impl Database {
    pub fn new() -> Self {
        Self {}
    }
}

pub fn get_connection() -> rusqlite::Connection {
    // let db_file = "~/.local/share/time-tracker/data-rs.db".resolve();
    let db_file = "~/.time_tracker/workspaces/default.db".resolve();
    let con = rusqlite::Connection::open(db_file).unwrap();
    con
}

pub fn start_entry(
    project_id: EntityId,
    description: String,
    task_id: Option<EntityId>,
) -> rusqlite::Result<TimeEntry> {
    // Logic to insert the time entry into the database
    let con: rusqlite::Connection = get_connection();
    let mut entry = TimeEntry::new(project_id, description, task_id);
    match con.execute(
        "INSERT INTO time_entries
            (project_id, task_id, description, start_time, end_time, created_at)
            VALUES (?, ?, ?, ?, NULL, ?)",
        (
            &entry.project_id,
            &entry.task_id,
            &entry.description,
            &entry.start_time,
            &entry.created_at,
        ),
    ) {
        Ok(_) => (),
        Err(_) => println!("Failed to start new entry!"),
    }

    entry.id = con.last_insert_rowid();
    Ok(entry)
}

pub fn stop_entry(entry_id: EntityId) -> rusqlite::Result<()> {
    let con = get_connection();
    let now = get_now_utc_py();
    con.execute(
        "UPDATE time_entries SET end_time = ? WHERE id = ? AND end_time IS NULL",
        (now, entry_id),
    )?;
    Ok(())
}

pub fn get_running_entry() -> Option<TimeEntry> {
    let con = get_connection();
    con.query_row(
        "SELECT id, project_id, task_id, description, start_time, end_time, created_at
             FROM time_entries WHERE end_time IS NULL ORDER BY start_time DESC LIMIT 1",
        [],
        |row| {
            Ok(TimeEntry {
                id: row.get(0)?,
                project_id: row.get(1)?,
                task_id: row.get(2)?,
                description: row.get(3)?,
                start_time: row.get(4)?,
                end_time: row.get(5).unwrap_or(None),
                created_at: row.get(6)?,
            })
        },
    )
    .optional()
    .unwrap_or(None)
}

pub fn get_recent_entries(limit: u32) -> Vec<TimeEntry> {
    let con = get_connection();

    let mut stmt = con
        .prepare(
            "SELECT id, project_id, task_id, description, start_time, end_time, created_at
         FROM time_entries WHERE end_time IS NOT NULL
         ORDER BY start_time DESC LIMIT ?",
        )
        .unwrap();

    let entries = stmt
        .query_map([limit], |row| {
            Ok(TimeEntry {
                id: row.get(0)?,
                project_id: row.get(1)?,
                task_id: row.get(2)?,
                description: row.get(3)?,
                start_time: row.get(4)?,
                end_time: row.get(5).unwrap_or(None),
                created_at: row.get(6)?,
            })
        })
        .unwrap();
    entries.filter_map(|e| e.ok()).collect()
}

pub fn test_db_queries() {
    let db_file = "~/.local/share/time-tracker/data-rs.db".resolve();
    println!("Opening: {}", db_file.display());

    // Check the file actually has content
    use std::fs;
    let meta = fs::metadata(&db_file).unwrap();
    println!("Size: {} bytes", meta.len());

    let con = get_connection();
    let count: i64 = con
        .query_row("SELECT COUNT(*) FROM time_entries", [], |r| r.get(0))
        .unwrap();
    println!("Total rows: {}", count);

    let count: i64 = con
        .query_row(
            // "SELECT COUNT(*) FROM time_entries WHERE end_time IS NULL",
            "SELECT COUNT(*)
             FROM time_entries WHERE end_time IS NULL ORDER BY start_time DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    println!("Rows with NULL end_time: {}", count);
}
