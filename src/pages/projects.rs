use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Layout;
use ratatui::style::{self, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, ListState, Padding, Paragraph, Row, Table,
    TableState,
};
use ratatui::{Frame, layout::Rect};
use ratatui::{layout::Constraint, style::Style};
use std::collections::HashMap;
use std::default;
use std::str::FromStr;

use crate::database::{self, EntityId};
use crate::pages::{KeyResult, PageAction, format_duration_hms_verbose};
use crate::theme;

pub enum ProjectAction {
    CreateProject {
        name: String,
        description: String,
    },
    UpdateProject {
        id: EntityId,
        name: String,
        description: String,
    },
}

struct CreateProjectForm {
    form: InputForm,
}
impl CreateProjectForm {
    fn default() -> Self {
        Self {
            form: InputForm::new(
                vec!["Name:".into(), "Description:".into()],
                vec!["add name..".into(), "add description..".into()],
            ),
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.form.render(frame, area);
    }
}

struct EditProjectForm {
    project: database::Project,
    form: InputForm,
}

impl EditProjectForm {
    fn new(project: database::Project) -> Self {
        let values = vec![project.name.clone(), project.description.clone()];
        Self {
            project,
            form: InputForm::with_values(
                vec!["Name:".into(), "Description:".into()],
                vec!["add name..".into(), "add description..".into()],
                values,
            ),
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.form.render(frame, area);
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> KeyResult<(EntityId, FormOutcome)> {
        match self.form.handle_key_event(key_event) {
            KeyResult::Unhandled => KeyResult::Unhandled,
            KeyResult::Consumed => KeyResult::Consumed,
            KeyResult::Action(outcome) => KeyResult::Action((self.project.id, outcome)),
        }
    }
}

enum PageState {
    Overview,
    CreateProject(CreateProjectForm),
    EditProjectInfo(EditProjectForm),
}

pub struct Project {
    projects: Vec<database::Project>,
    metrics_per_project: HashMap<EntityId, database::IntervalMetrics>,
    page_state: PageState,
    table_state: TableState,
}

impl Project {
    pub fn new() -> Self {
        Self {
            projects: Vec::new(),
            metrics_per_project: HashMap::new(),
            page_state: PageState::Overview,
            table_state: TableState::default(),
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
        // let block = Block::new().padding(Padding::symmetric(1, 1));

        // self.render_project_list(frame, block.inner(area));
        self.render_project_list(frame, area);

        match &mut self.page_state {
            PageState::CreateProject(form) => {
                let center_area = area.centered(Constraint::Length(50), Constraint::Length(20));
                frame.render_widget(Clear, center_area);
                let create_project_block = Block::bordered()
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .bg(theme::BASE);
                let form_area = create_project_block.inner(center_area);

                frame.render_widget(&create_project_block, center_area);
                form.render(frame, form_area);
            }
            PageState::EditProjectInfo(form) => {
                let center_area = area.centered(Constraint::Length(50), Constraint::Length(20));
                frame.render_widget(Clear, center_area);
                let create_project_block = Block::bordered()
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .bg(theme::BASE);
                let form_area = create_project_block.inner(center_area);

                frame.render_widget(&create_project_block, center_area);
                form.render(frame, form_area);
            }
            PageState::Overview => (),
        }
    }

    fn render_project_list(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let [title_area, entries_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(area);
        frame.render_widget(Paragraph::new(" Projects"), title_area);
        let table_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);
        frame.render_widget(&table_block, entries_area);
        let table_area = table_block.inner(entries_area);

        let headings = Row::new(
            ["Name", "Description", "Total Hours"]
                .into_iter()
                .map(|t| Cell::from(t).bold().fg(theme::ROSEWATER)),
        )
        .underlined();
        let rows = self.projects.iter().enumerate().map(|(i, p)| {
            let metrics = self.metrics_per_project.get(&p.id);

            let row_bg_color = if i % 2 == 0 {
                theme::BASE
            } else {
                theme::SURFACE_0
            };
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
            .bg(row_bg_color)
        });
        let column_widths = vec![
            Constraint::Length(15),
            Constraint::Min(7),
            Constraint::Length(10),
            Constraint::Length(22),
        ];
        let table = Table::new(rows, column_widths)
            .header(headings)
            .row_highlight_style(style::Style::new().fg(theme::BASE).bg(theme::PINK))
            .cell_highlight_style(style::Style::new().fg(theme::BASE).bg(theme::LAVENDER));
        frame.render_stateful_widget(table, table_area, &mut self.table_state);
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> KeyResult<PageAction> {
        match &mut self.page_state {
            PageState::Overview => self.handle_main_page_key_event(key_event),
            PageState::CreateProject(form) => {
                let form_outcome = form.form.handle_key_event(key_event);

                match form_outcome {
                    KeyResult::Action(FormOutcome::Submitted(fields)) => {
                        self.page_state = PageState::Overview;
                        return KeyResult::Action(PageAction::Projects(
                            ProjectAction::CreateProject {
                                name: fields[0].to_owned(),
                                description: fields[1].to_owned(),
                            },
                        ));
                    }
                    KeyResult::Action(FormOutcome::Cancelled) => {
                        self.page_state = PageState::Overview;
                        KeyResult::Consumed
                    }
                    KeyResult::Unhandled => KeyResult::Unhandled,
                    KeyResult::Consumed => KeyResult::Consumed,
                }
            }
            PageState::EditProjectInfo(form) => {
                let form_outcome = form.handle_key_event(key_event);

                match form_outcome {
                    KeyResult::Action((id, FormOutcome::Submitted(fields))) => {
                        self.page_state = PageState::Overview;
                        return KeyResult::Action(PageAction::Projects(
                            ProjectAction::UpdateProject {
                                id,
                                name: fields[0].to_owned(),
                                description: fields[1].to_owned(),
                            },
                        ));
                    }
                    KeyResult::Action((id, FormOutcome::Cancelled)) => {
                        self.page_state = PageState::Overview;
                        KeyResult::Consumed
                    }
                    KeyResult::Unhandled => KeyResult::Unhandled,
                    KeyResult::Consumed => KeyResult::Consumed,
                }
            }
        }
    }
    fn handle_main_page_key_event(&mut self, key_event: KeyEvent) -> KeyResult<PageAction> {
        match key_event.code {
            KeyCode::Char('a') => {
                self.page_state = PageState::CreateProject(CreateProjectForm::default());
                KeyResult::Consumed
            }
            KeyCode::Char('e') if self.table_state.selected().is_some() => {
                let row = self.table_state.selected().unwrap();
                let project = self.projects.get(row).unwrap().clone();
                self.page_state = PageState::EditProjectInfo(EditProjectForm::new(project));
                KeyResult::Consumed
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.table_state.selected().is_none() {
                    self.table_state.select(Some(0));
                } else {
                    self.table_state.select_previous();
                }
                KeyResult::Consumed
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.table_state.selected().is_none() {
                    self.table_state.select(Some(0));
                } else {
                    self.table_state.select_next();
                }
                KeyResult::Consumed
            }
            KeyCode::Enter => {
                // Enter project name or description
                KeyResult::Consumed
            }
            KeyCode::Char('d') if self.table_state.selected().is_some() => {
                let row = self.table_state.selected().unwrap();
                let project_id = self.projects.get(row).unwrap().id;
                // return Some(ProjectAction::DeleteProject(project_id));
                KeyResult::Consumed
            }
            KeyCode::Char(' ') => {
                self.page_state = PageState::Overview;
                KeyResult::Consumed
            }
            _ => KeyResult::Unhandled,
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

enum FormOutcome {
    Submitted(Vec<String>),
    Cancelled,
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

    fn with_values(labels: Vec<String>, placeholders: Vec<String>, values: Vec<String>) -> Self {
        let mut values = values;
        values.resize(labels.len(), "".to_string());
        Self {
            list_state: ListState::default().with_selected(Some(0)),
            labels,
            placeholders,
            values,
            active_input: None,
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> KeyResult<FormOutcome> {
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
                        KeyResult::Consumed
                    }
                    _ => {
                        match key_event.code {
                            KeyCode::Char(c) => {
                                self.values.get_mut(field_idx).unwrap().push(c);
                                KeyResult::Consumed
                            }
                            KeyCode::Backspace => {
                                self.values.get_mut(field_idx).unwrap().pop();
                                KeyResult::Consumed
                            }
                            // Todo: Esc should revert to old value
                            KeyCode::Esc | KeyCode::Enter => {
                                self.active_input = None;
                                KeyResult::Consumed
                            }
                            KeyCode::Tab => {
                                self.list_state.select_next();
                                self.active_input = self.list_state.selected();
                                KeyResult::Consumed
                            }
                            _ => KeyResult::Unhandled,
                        }
                    }
                }
            }
            None => match key_event.code {
                KeyCode::Char('k') | KeyCode::Up => {
                    self.list_state.select_previous();
                    KeyResult::Consumed
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.list_state.select_next();
                    KeyResult::Consumed
                }
                KeyCode::Enter => {
                    self.active_input = self.list_state.selected();
                    KeyResult::Consumed
                }
                KeyCode::Esc => KeyResult::Action(FormOutcome::Cancelled),
                KeyCode::Char(' ') => {
                    KeyResult::Action(FormOutcome::Submitted(self.values.to_owned()))
                }
                _ => KeyResult::Unhandled,
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
