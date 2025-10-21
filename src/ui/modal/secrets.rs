use crate::{
    app::{Action, AppContext, AppResult, AppView},
    components::Component,
    storage::SecretKey,
};
use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::cmp::min;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecretsField {
    Etherscan,
    Anvil,
}

impl Default for SecretsField {
    fn default() -> Self {
        SecretsField::Etherscan
    }
}

#[derive(Debug, Clone)]
pub enum SecretsFormCommand {
    FocusNextField,
    FocusPreviousField,
    InputChar(char),
    InsertText(String),
    Backspace,
    Submit,
    Cancel,
    ClearField,
}

#[derive(Debug, Default)]
pub struct SecretsModal {
    etherscan_value: String,
    anvil_value: String,
    focused_field: SecretsField,
    message: Option<String>,
}

impl SecretsModal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn command_from_key(event: KeyEvent) -> Option<SecretsFormCommand> {
        use crossterm::event::{KeyCode, KeyModifiers};
        match (event.modifiers, event.code) {
            (_, KeyCode::Esc) => Some(SecretsFormCommand::Cancel),
            (KeyModifiers::NONE, KeyCode::Tab) | (KeyModifiers::NONE, KeyCode::Down) => {
                Some(SecretsFormCommand::FocusNextField)
            }
            (KeyModifiers::SHIFT, KeyCode::Tab) | (KeyModifiers::NONE, KeyCode::Up) => {
                Some(SecretsFormCommand::FocusPreviousField)
            }
            (_, KeyCode::Enter) => Some(SecretsFormCommand::Submit),
            (_, KeyCode::Backspace) => Some(SecretsFormCommand::Backspace),
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => Some(SecretsFormCommand::ClearField),
            (modifiers, KeyCode::Char(c)) if !modifiers.contains(KeyModifiers::CONTROL) => {
                Some(SecretsFormCommand::InputChar(c))
            }
            _ => None,
        }
    }

    fn selected_value(&mut self) -> &mut String {
        match self.focused_field {
            SecretsField::Etherscan => &mut self.etherscan_value,
            SecretsField::Anvil => &mut self.anvil_value,
        }
    }

    fn field_title(field: SecretsField) -> &'static str {
        match field {
            SecretsField::Etherscan => "Etherscan API Key",
            SecretsField::Anvil => "Anvil RPC URL",
        }
    }

    fn cycle_field(&mut self, forward: bool) {
        self.focused_field = if forward {
            match self.focused_field {
                SecretsField::Etherscan => SecretsField::Anvil,
                SecretsField::Anvil => SecretsField::Etherscan,
            }
        } else {
            match self.focused_field {
                SecretsField::Etherscan => SecretsField::Anvil,
                SecretsField::Anvil => SecretsField::Etherscan,
            }
        };
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.etherscan_value.trim().is_empty() {
            return Err("Etherscan API key is required");
        }
        if self.anvil_value.trim().is_empty() {
            return Err("Anvil RPC URL is required");
        }
        Ok(())
    }

    fn save(&mut self, ctx: &mut AppContext<'_>) -> AppResult<Option<Action>> {
        if let Err(message) = self.validate() {
            self.message = Some(message.to_string());
            return Ok(None);
        }

        let etherscan = self.etherscan_value.trim();
        let anvil = self.anvil_value.trim();

        ctx.storage
            .secrets()
            .set(SecretKey::EtherscanApiKey, etherscan)?;
        ctx.storage.secrets().set(SecretKey::AnvilRpcUrl, anvil)?;

        ctx.state.secrets.etherscan_api_key = Some(etherscan.to_string());
        ctx.state.secrets.anvil_rpc_url = Some(anvil.to_string());
        self.message = Some("Configuration saved".into());
        Ok(Some(Action::SecretsSaved))
    }

    fn clear_field(&mut self) {
        self.selected_value().clear();
    }

    fn apply_command(
        &mut self,
        command: &SecretsFormCommand,
        ctx: &mut AppContext<'_>,
    ) -> AppResult<Option<Action>> {
        match command {
            SecretsFormCommand::FocusNextField => {
                self.message = None;
                self.cycle_field(true);
            }
            SecretsFormCommand::FocusPreviousField => {
                self.message = None;
                self.cycle_field(false);
            }
            SecretsFormCommand::InputChar(c) => {
                self.message = None;
                self.selected_value().push(*c);
            }
            SecretsFormCommand::InsertText(text) => {
                self.message = None;
                let cleaned: String = text
                    .chars()
                    .filter(|ch| !matches!(ch, '\r' | '\n'))
                    .collect();
                self.selected_value().push_str(&cleaned);
            }
            SecretsFormCommand::Backspace => {
                self.message = None;
                self.selected_value().pop();
            }
            SecretsFormCommand::ClearField => {
                self.message = None;
                self.clear_field();
            }
            SecretsFormCommand::Submit => return self.save(ctx),
            SecretsFormCommand::Cancel => return Ok(Some(Action::CloseModal)),
        }
        Ok(None)
    }

    fn centered_rect(&self, width: u16, height: u16, area: Rect) -> Rect {
        let width = min(width, area.width);
        let height = min(height, area.height);
        Rect {
            x: area.x + (area.width.saturating_sub(width)) / 2,
            y: area.y + (area.height.saturating_sub(height)) / 2,
            width,
            height,
        }
    }
}

impl Component for SecretsModal {
    type Command = SecretsFormCommand;

    fn init(&mut self, ctx: &mut AppContext<'_>) -> AppResult<()> {
        self.etherscan_value = ctx
            .state
            .secrets
            .etherscan_api_key
            .clone()
            .unwrap_or_default();
        self.anvil_value = ctx.state.secrets.anvil_rpc_url.clone().unwrap_or_default();
        Ok(())
    }

    fn update(
        &mut self,
        command: &Self::Command,
        ctx: &mut AppContext<'_>,
    ) -> AppResult<Option<Action>> {
        self.apply_command(command, ctx)
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &AppView<'_>) {
        let modal_area = self.centered_rect(72, 15, area);
        frame.render_widget(Clear, modal_area);

        let title = if ctx.state.secrets.etherscan_api_key.is_some()
            && ctx.state.secrets.anvil_rpc_url.is_some()
        {
            "Update Configuration"
        } else {
            "Configuration Required"
        };

        let block = Block::default()
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray));

        let inner = block.inner(modal_area);
        frame.render_widget(block, modal_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(2),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(1),
                    Constraint::Length(2),
                ]
                .as_ref(),
            )
            .split(inner);

        let intro = Paragraph::new(Text::raw(
            "Enter credentials to enable contract lookups and local RPC calls.",
        ))
        .alignment(Alignment::Center);
        frame.render_widget(intro, chunks[0]);

        for (idx, (field, target_area)) in [
            (SecretsField::Etherscan, chunks[1]),
            (SecretsField::Anvil, chunks[2]),
        ]
        .into_iter()
        .enumerate()
        {
            let value = match field {
                SecretsField::Etherscan => &self.etherscan_value,
                SecretsField::Anvil => &self.anvil_value,
            };
            let placeholder = if value.trim().is_empty() {
                "<required>"
            } else {
                value
            };
            let is_focused = self.focused_field == field;
            let mut spans = Vec::new();
            spans.push(Span::styled(
                format!("{}: ", SecretsModal::field_title(field)),
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                placeholder.to_string(),
                if is_focused {
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Blue)
                        .add_modifier(Modifier::BOLD)
                } else if value.trim().is_empty() {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::White)
                },
            ));
            if is_focused {
                spans.push(Span::styled(
                    " ▌",
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            let paragraph = Paragraph::new(Line::from(spans))
                .block(Block::default().borders(Borders::NONE))
                .alignment(Alignment::Left);
            frame.render_widget(paragraph, target_area);

            if idx == 0 {
                let hint = Paragraph::new(Line::from(Span::styled(
                    "Rotate fields with Tab • Clear with Ctrl+U",
                    Style::default().fg(Color::Gray),
                )))
                .alignment(Alignment::Left);
                frame.render_widget(hint, chunks[3]);
            }
        }

        let status_line = if let Some(message) = self.message.as_ref() {
            Paragraph::new(Span::styled(
                message.clone(),
                Style::default().fg(Color::Yellow),
            ))
        } else {
            Paragraph::new(Span::styled(
                "Submit with Enter. Cancel with Esc.",
                Style::default().fg(Color::Gray),
            ))
        };
        frame.render_widget(status_line, chunks[4]);
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>) -> AppResult<Option<Action>> {
        Ok(None)
    }
}
