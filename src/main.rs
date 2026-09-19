use color_eyre::eyre::Result;
use core::time::Duration;
use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::DisableMouseCapture;
use ratatui::crossterm::event::EnableMouseCapture;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use ratatui::prelude::*;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::Span,
    widgets::Block,
    widgets::Paragraph,
};
struct App {
    exit: bool,
}

impl App {
    pub fn new() -> Self {
        Self { exit: false }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }
    fn draw(&self, frame: &mut Frame) {
        let block = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("TimeTracker");
        let area = frame.area();
        let inner_area = block.inner(area);
        let constraints = Constraint::from_percentages([25, 75]);
        let splits = Layout::default().constraints(constraints).split(inner_area);
        let top = splits[0];
        let bottom = splits[1];

        let navbar = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("Navigation");
        let text = Self::create_navbar(vec!["Home", "Projects"]);
        frame.render_widget(Paragraph::new(text).block(navbar), top);

        let page = Block::bordered()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("Page");
        frame.render_widget(page, bottom);
    }

    fn create_navbar(items: Vec<&str>) -> Line {
        let styled_items: Vec<Span> = items
            .iter()
            .map(|item| {
                let mut chars = item.chars();
                let first_char = chars.next().unwrap_or(' ');
                let rest: String = chars.collect();
                vec![
                    Span::styled(
                        first_char.to_string(),
                        Style::default().add_modifier(Modifier::UNDERLINED),
                    ), // Underlined letter
                    Span::raw(rest),
                    Span::raw(" | ".to_string()),
                ]
            })
            .flatten()
            .collect();
        Line::from(styled_items)
    }
    fn handle_events(&mut self) -> Result<()> {
        let timeout = Duration::from_secs_f64(1.0 / 50.0);
        if !event::poll(timeout)? {
            return Ok(());
        }
        match event::read()? {
            Event::Key(key_event) => self.handle_key_event(key_event)?,
            _ => (),
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Char('q') => {
                println!("Exiting");
                self.exit = true;
            }
            _ => (),
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout(); // This is a special case. Normally using stdout is fine
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    App::new().run(&mut terminal)?;

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
