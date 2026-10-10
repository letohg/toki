mod app;
mod database;
mod pages;
mod theme;

use crate::app::App;
use crate::pages::format_duration_hms_verbose;
use clap::Parser;

use color_eyre::eyre::Result;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::DisableMouseCapture;
use ratatui::crossterm::event::EnableMouseCapture;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use resolve_path::PathResolveExt;
use std::fs::File;
use std::io;
use std::io::BufReader;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    /// Get the active timer status
    #[arg(short, long)]
    status: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.status {
        const CONFIG_DIR: &str = "~/.time_tracker";
        let settings_file = format!("{}/settings.json", CONFIG_DIR);

        let db_path = if matches!(std::fs::exists(&settings_file), Ok(true)) {
            let file = File::open(settings_file.resolve()).unwrap();
            let reader = BufReader::new(file);
            let settings: serde_json::Value = serde_json::from_reader(reader).unwrap();
            let id = settings["active_workspace_id"].as_str().unwrap();
            format!("{CONFIG_DIR}/workspaces/{}.db", id)
        } else {
            format!("{CONFIG_DIR}/workspaces/default.db")
        };
        let db = database::Database::new(db_path)?;
        let projects = db.get_projects()?;
        let active_entry = db.get_running_entry()?;

        if let Some(e) = active_entry
            && let Some(p) = &projects.iter().find(|p| p.id == e.project_id)
        {
            println!("{}: {}", p.name, format_duration_hms_verbose(&e.duration()));
        } else {
            println!("idle");
        }
        return Ok(());
    }
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout(); // This is a special case. Normally using stdout is fine
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    match App::new().run(&mut terminal) {
        Ok(_) => (),
        Err(_) => (),
    };

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
