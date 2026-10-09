use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Layout;
use ratatui::style::{self, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, ListState, Padding, Paragraph, Row, Table,
};
use ratatui::{Frame, layout::Rect};
use ratatui::{layout::Constraint, style::Style};
use std::collections::HashMap;
use std::default;
use std::str::FromStr;

use crate::database::{self, EntityId};
use crate::pages::{PageAction, format_duration_hms_verbose};
use crate::theme;

pub enum ProjectAction {
    CreateProject { name: String, description: String },
}

#[derive(Default)]
struct NewProjectInfo {
    name: String,
    description: String,
}

struct CreateProjectState {
    input_form: InputForm,
}
impl CreateProjectState {
    fn default() -> Self {
        Self {
            input_form: InputForm::new(
                vec!["Name:".into(), "Description:".into()],
                vec!["add name..".into(), "add description..".into()],
            ),
        }
    }
}

enum PageState {
    Overview,
    CreateProject(CreateProjectState),
}

pub struct Project {
    projects: Vec<database::Project>,
    metrics_per_project: HashMap<EntityId, database::IntervalMetrics>,
    page_state: PageState,
}

impl Project {
    pub fn new() -> Self {
        Self {
            projects: Vec::new(),
            metrics_per_project: HashMap::new(),
            page_state: PageState::Overview,
        }
    }

    pub fn refresh(
        &mut self,
        projects: Vec<database::Project>,
        metrics_per_project: HashMap<EntityId, database::IntervalMetrics>,
    ) {
        self.projects = projects;
        self.metrics_per_project = metrics_per_project;
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::new().padding(Padding::symmetric(1, 1));

        self.render_project_list(frame, block.inner(area));

        match &mut self.page_state {
            PageState::CreateProject(state) => {
                let center_area = area.centered(Constraint::Length(50), Constraint::Length(20));
                frame.render_widget(Clear, center_area);
                let create_project_block = Block::bordered()
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .bg(theme::BASE);

                frame.render_widget(&create_project_block, center_area);
                state
                    .input_form
                    .render(frame, create_project_block.inner(center_area));

                // let [lables_area, inputs_area] =
                //     Layout::horizontal([Constraint::Length(14), Constraint::Min(6)])
                //         .areas(create_project_block.inner(center_area));
                // frame.render_widget(create_project_block, center_area);
                //
                // let labels = List::new(vec!["Name:", "Description:"]);
                // frame.render_stateful_widget(labels, lables_area, &mut state.list_state);
                //
                // let project_name =
                //     create_input_list_item("add name...", Some(state.new_project.name.clone()));
                // let project_description = create_input_list_item(
                //     "add name...",
                //     Some(state.new_project.description.clone()),
                // );
                //
                // frame.render_stateful_widget(
                //     List::new(vec![project_name, project_description])
                //         .highlight_style(Style::default().fg(theme::LAVENDER).bold())
                //         .highlight_symbol("› "),
                //     inputs_area,
                //     &mut state.list_state,
                // );
            }
            PageState::Overview => (),
        }
    }

    fn render_project_list(&self, frame: &mut Frame<'_>, area: Rect) {
        let [title_area, entries_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(area);
        frame.render_widget(Paragraph::new("Projects"), title_area);
        let table_block = Block::new().borders(Borders::TOP);
        frame.render_widget(&table_block, entries_area);
        let table_area = table_block.inner(entries_area);

        let headings = Row::new(
            ["Name", "Description", "Total Hours"]
                .into_iter()
                .map(|t| Cell::from(t).bold()),
        );
        let rows = self.projects.iter().map(|p| {
            let metrics = self.metrics_per_project.get(&p.id);
            Row::new(vec![
                Cell::from(p.name.clone())
                    .style(Style::new().fg(style::Color::from_str(&p.color).unwrap())),
                Cell::from(p.description.clone()),
                Cell::from(metrics.map_or("--:--".to_string(), |m| {
                    format_duration_hms_verbose(&m.total_time)
                })),
                Cell::from(
                    Line::from(metrics.map_or("in 0 sessions".to_string(), |m| {
                        format!("in {:2} sessions", m.session_count)
                    }))
                    .right_aligned(),
                ),
            ])
        });
        let column_widths = vec![
            Constraint::Length(15),
            Constraint::Min(7),
            Constraint::Length(10),
            Constraint::Length(22),
        ];
        let table = Table::new(rows, column_widths)
            .header(headings)
            .style(Style::new().add_modifier(style::Modifier::BOLD));
        frame.render_widget(table, table_area);
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<PageAction> {
        match &mut self.page_state {
            PageState::Overview => self.handle_main_page_key_event(key_event),
            PageState::CreateProject(state) => {
                let action = state.input_form.handle_key_event(key_event);
                if let Some(a) = action {
                    self.page_state = PageState::Overview;
                    return Some(PageAction::Projects(a));
                }
                true
            } //     match key_event.code {
              //     KeyCode::Char('k') | KeyCode::Up => {
              //         state.list_state.select_previous();
              //         true
              //     }
              //     KeyCode::Char('j') | KeyCode::Down => {
              //         state.list_state.select_next();
              //         true
              //     }
              //     KeyCode::Enter => {
              //         // Enter project name or description
              //         true
              //     }
              //     KeyCode::Char('d') | KeyCode::Char(' ') => {
              //         self.page_state = PageState::Overview;
              //         true
              //     }
              //     _ => false,
              // },
        };

        None
    }
    fn handle_main_page_key_event(&mut self, key_event: KeyEvent) -> bool {
        match key_event.code {
            KeyCode::Char('a') => {
                self.page_state = PageState::CreateProject(CreateProjectState::default());
                true
            }
            _ => false,
        }
    }
    // fn handle_create_project_form_key_event(
    //     &mut self,
    //     key_event: KeyEvent,
    //     state: &mut CreateProjectState,
    // ) -> bool {
    //     match key_event.code {
    //         KeyCode::Char('k') | KeyCode::Up => {
    //             state.list_state.select_previous();
    //             true
    //         }
    //         KeyCode::Char('j') | KeyCode::Down => {
    //             state.list_state.select_next();
    //             true
    //         }
    //         KeyCode::Enter => {
    //             self.page_state = PageState::Overview;
    //             true
    //         }
    //         _ => false,
    //     }
    // }
}

fn create_input_list_item(placeholder: &str, content: Option<String>) -> ListItem<'_> {
    ListItem::new(Line::from(vec![match content {
        None => Span::raw(placeholder).italic().fg(theme::OVERLAY_0),
        Some(c) if c.is_empty() => Span::raw(placeholder).italic().fg(theme::OVERLAY_0),
        Some(c) => Span::raw(c),
    }]))
}

struct InputForm {
    list_state: ListState,
    labels: Vec<String>,
    placeholders: Vec<String>,
    values: Vec<String>,
    active_input: Option<usize>,
}

impl InputForm {
    fn new(labels: Vec<String>, placeholders: Vec<String>) -> Self {
        let mut values = Vec::new();
        values.resize(labels.len(), "".to_string());
        Self {
            list_state: ListState::default().with_selected(Some(0)),
            labels,
            placeholders,
            values,
            active_input: None,
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<ProjectAction> {
        match self.active_input {
            Some(field_idx) => {
                match key_event {
                    KeyEvent {
                        code: KeyCode::Tab,
                        modifiers: KeyModifiers::SHIFT,
                        ..
                    } => {
                        self.list_state.select_previous();
                        self.active_input = self.list_state.selected();
                    }
                    _ => {
                        match key_event.code {
                            KeyCode::Char(c) => self.values.get_mut(field_idx).unwrap().push(c),
                            KeyCode::Backspace => {
                                self.values.get_mut(field_idx).unwrap().pop();
                            }
                            // Todo: Esc should revert to old value
                            KeyCode::Esc | KeyCode::Enter => self.active_input = None,
                            KeyCode::Tab => {
                                self.list_state.select_next();
                                self.active_input = self.list_state.selected();
                            }
                            _ => (),
                        }
                    }
                };
                None
            }
            None => match key_event.code {
                KeyCode::Char('k') | KeyCode::Up => {
                    self.list_state.select_previous();
                    None
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.list_state.select_next();
                    None
                }
                KeyCode::Enter => {
                    self.active_input = self.list_state.selected();
                    None
                }
                KeyCode::Char(' ') => Some(ProjectAction::CreateProject {
                    name: self.values[0].clone(),
                    description: self.values[1].clone(),
                }),
                _ => None,
            },
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let [lables_area, inputs_area] =
            Layout::horizontal([Constraint::Length(14), Constraint::Min(6)]).areas(area);

        let labels = List::new(self.labels.clone());
        frame.render_stateful_widget(labels, lables_area, &mut self.list_state);

        let inputs: List = self
            .values
            .iter()
            .zip(&self.placeholders)
            .enumerate()
            .map(|(i, (v, p))| {
                if let Some(active) = self.active_input
                    && active == i
                {
                    ListItem::from(format!("{}│", v))
                } else {
                    create_input_list_item(p, Some(v.clone()))
                }
            })
            .collect();

        frame.render_stateful_widget(
            inputs
                .highlight_style(Style::default().fg(theme::LAVENDER).bold())
                .highlight_symbol("› "),
            inputs_area,
            &mut self.list_state,
        );
    }
}
