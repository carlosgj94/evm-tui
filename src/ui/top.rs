use super::util::short_hex;
use crate::{
    app::{
        Action, AddressRef, AppContext, AppResult, AppView, FocusedPane, Message, SelectedEntity,
        TransactionRef,
    },
    components::Component,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use tokio::time::{Duration, sleep};

#[derive(Debug)]
pub struct TopBar {
    title: String,
    search_active: bool,
    search_value: String,
    pending_search: bool,
    status: Option<String>,
}

impl Default for TopBar {
    fn default() -> Self {
        Self {
            title: "evm-tui".to_string(),
            search_active: false,
            search_value: String::new(),
            pending_search: false,
            status: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TopCommand {
    ActivateSearch,
    InputChar(char),
    Backspace,
    Submit,
    Cancel,
    SearchCompleted {
        query: String,
        entity: SelectedEntity,
    },
    SearchFailed {
        query: String,
        error: String,
    },
    ShowStatus(String),
}

impl TopBar {
    const LAST_QUERY_KEY: &'static str = "top:last_query";

    pub fn is_search_active(&self) -> bool {
        self.search_active
    }

    fn decode_query(query: &str) -> Result<SelectedEntity, String> {
        let trimmed = query.trim();
        let lower = trimmed.trim();
        let (prefix_stripped, has_prefix) = if let Some(rest) = lower.strip_prefix("0x") {
            (rest, true)
        } else {
            (lower, false)
        };
        if prefix_stripped.is_empty() {
            return Err("Empty query".into());
        }
        if prefix_stripped.len() == 40 && prefix_stripped.chars().all(|c| c.is_ascii_hexdigit()) {
            let address = if has_prefix {
                format!("0x{prefix_stripped}")
            } else {
                format!("0x{prefix_stripped}")
            };
            let short = short_hex(&address);
            return Ok(SelectedEntity::Address(AddressRef {
                label: format!("Address {short}"),
                address,
                chain: "Mainnet".into(),
            }));
        }
        if prefix_stripped.len() == 64 && prefix_stripped.chars().all(|c| c.is_ascii_hexdigit()) {
            let hash = if has_prefix {
                format!("0x{prefix_stripped}")
            } else {
                format!("0x{prefix_stripped}")
            };
            return Ok(SelectedEntity::Transaction(TransactionRef {
                label: format!("Txn {}", short_hex(&hash)),
                hash,
                chain: "Mainnet".into(),
            }));
        }
        Err("Input could not be decoded as a valid address or transaction".into())
    }

    fn status_line(&self) -> Option<Line<'_>> {
        self.status
            .as_ref()
            .map(|status| Line::from(status.clone()).style(Style::default().fg(Color::Gray)))
    }
}

impl Component for TopBar {
    type Command = TopCommand;

    fn init(&mut self, ctx: &mut AppContext<'_>) -> AppResult<()> {
        if let Some(raw) = ctx.storage.settings().get(Self::LAST_QUERY_KEY)? {
            if let Ok(value) = String::from_utf8(raw) {
                if !value.is_empty() {
                    self.search_value = value;
                }
            }
        }
        Ok(())
    }

    fn update(
        &mut self,
        command: &Self::Command,
        ctx: &mut AppContext<'_>,
    ) -> AppResult<Option<Action>> {
        match command {
            TopCommand::ActivateSearch => {
                self.search_active = true;
                self.pending_search = false;
                self.status = Some("Type an address or transaction hash".into());
            }
            TopCommand::InputChar(c) => {
                if !self.search_active {
                    self.search_active = true;
                }
                self.search_value.push(*c);
            }
            TopCommand::Backspace => {
                self.search_value.pop();
            }
            TopCommand::Submit => {
                let query = self.search_value.trim().to_string();
                if query.is_empty() {
                    self.status = Some("Enter a value to search".into());
                    return Ok(None);
                }
                self.pending_search = true;
                let commands = ctx.commands.clone();
                let query_for_task = query.clone();
                commands.spawn_async(move || {
                    let query_clone = query_for_task.clone();
                    async move {
                        sleep(Duration::from_millis(400)).await;
                        match TopBar::decode_query(&query_clone) {
                            Ok(entity) => Message::SearchCompleted {
                                query: query_clone,
                                entity,
                            },
                            Err(error) => Message::SearchFailed {
                                query: query_clone,
                                error,
                            },
                        }
                    }
                });
                self.status = Some(format!("Searching for {query}…"));
                return Ok(Some(Action::LoadingStarted(FocusedPane::Top)));
            }
            TopCommand::Cancel => {
                self.search_active = false;
                self.pending_search = false;
                self.status = Some("Search cancelled".into());
            }
            TopCommand::SearchCompleted { query, entity } => {
                self.pending_search = false;
                self.status = Some(match entity {
                    SelectedEntity::Address(addr) => {
                        format!("Loaded address {}", short_hex(&addr.address))
                    }
                    SelectedEntity::Transaction(tx) => {
                        format!("Loaded transaction {}", short_hex(&tx.hash))
                    }
                });
                self.search_value = query.clone();
                self.search_active = false;
                ctx.storage
                    .settings()
                    .put(Self::LAST_QUERY_KEY, query.as_bytes())?;
            }
            TopCommand::SearchFailed { query, error } => {
                self.pending_search = false;
                self.status = Some(format!("Failed to load {}: {}", short_hex(query), error));
            }
            TopCommand::ShowStatus(message) => {
                self.status = Some(message.clone());
                self.search_active = false;
                self.pending_search = false;
            }
        }
        Ok(None)
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &AppView<'_>) {
        let is_focused = matches!(ctx.state.navigation.focused_pane, FocusedPane::Top);
        let descriptor = match &ctx.state.selected {
            Some(entity @ SelectedEntity::Address(addr)) => {
                let marker = if ctx.state.is_favorite(entity) {
                    " *"
                } else {
                    ""
                };
                format!("{} [{}]{}", short_hex(&addr.address), addr.chain, marker)
            }
            Some(entity @ SelectedEntity::Transaction(tx)) => {
                let marker = if ctx.state.is_favorite(entity) {
                    " *"
                } else {
                    ""
                };
                format!("{} ({}){}", short_hex(&tx.hash), tx.chain, marker)
            }
            None => "No selection".to_string(),
        };
        let title = Line::from(format!("[1] {} • {}", self.title, descriptor));
        let style = if is_focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };

        let mut lines = Vec::new();
        if self.search_active {
            let prompt_style = if self.pending_search {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            let hint = Span::styled(
                format!("› {}_", self.search_value),
                prompt_style.add_modifier(Modifier::BOLD),
            );
            lines.push(Line::from(vec![hint]));
            lines.push(Line::from("Enter to submit • Esc to cancel"));
        } else {
            lines.push(Line::from("Press / to search addresses or transactions"));
        }
        if let Some(status) = self.status_line() {
            lines.push(status);
        }
        let missing_etherscan = ctx.state.secrets.etherscan_api_key.is_none();
        let missing_anvil = ctx.state.secrets.anvil_rpc_url.is_none();
        if missing_etherscan || missing_anvil {
            let mut parts = Vec::new();
            if missing_etherscan {
                parts.push("ETHERSCAN_API_KEY");
            }
            if missing_anvil {
                parts.push("ANVIL_RPC_URL");
            }
            let warning = format!("Missing config: {}", parts.join(", "));
            lines.push(Line::from(Span::styled(
                warning,
                Style::default().fg(Color::Yellow),
            )));
        }

        let widget = Paragraph::new(lines)
            .style(Style::default().fg(Color::Gray))
            .block(Block::bordered().title(title.style(style)));
        frame.render_widget(widget, area);
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>) -> AppResult<Option<Action>> {
        Ok(None)
    }
}
