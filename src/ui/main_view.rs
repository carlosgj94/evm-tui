use super::util::short_hex;
use crate::{
    app::{
        Action, AppContext, AppResult, AppView, FocusedPane, MainViewMode, MainViewTab,
        SelectedEntity,
    },
    components::Component,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Tabs},
};

#[derive(Debug, Default)]
pub struct MainView {
    placeholder: String,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum MainViewCommand {
    SetMode(MainViewMode),
    SwitchTab(MainViewTab),
    NextTab,
    PreviousTab,
    HydrationStarted,
    HydrationFinished,
}

impl MainView {
    fn tab_titles(mode: MainViewMode) -> &'static [(&'static str, MainViewTab)] {
        match mode {
            MainViewMode::Address => &[
                ("Transactions", MainViewTab::AddressTransactions),
                ("Internal", MainViewTab::AddressInternal),
                ("Balances", MainViewTab::AddressBalances),
                ("Permissions", MainViewTab::AddressPermissions),
            ],
            MainViewMode::Transaction => &[
                ("Summary", MainViewTab::TransactionSummary),
                ("Debug", MainViewTab::TransactionDebug),
                ("Storage Diff", MainViewTab::TransactionStorageDiff),
            ],
        }
    }

    fn tab_index(mode: MainViewMode, tab: MainViewTab) -> usize {
        Self::tab_titles(mode)
            .iter()
            .position(|(_, t)| *t == tab.normalize(mode))
            .unwrap_or(0)
    }

    fn content_for(tab: MainViewTab) -> &'static str {
        match tab {
            MainViewTab::AddressTransactions => "Address transactions overview (placeholder)",
            MainViewTab::AddressInternal => "Address internal calls (placeholder)",
            MainViewTab::AddressBalances => "Address balances summary (placeholder)",
            MainViewTab::AddressPermissions => "Address permissions matrix (placeholder)",
            MainViewTab::TransactionSummary => "Transaction summary (placeholder)",
            MainViewTab::TransactionDebug => "Transaction debugger (placeholder)",
            MainViewTab::TransactionStorageDiff => "Transaction storage diff (placeholder)",
        }
    }
}

impl Component for MainView {
    type Command = MainViewCommand;

    fn init(&mut self, _ctx: &mut AppContext<'_>) -> AppResult<()> {
        self.placeholder = "Select a favorite to begin".into();
        Ok(())
    }

    fn update(
        &mut self,
        command: &Self::Command,
        ctx: &mut AppContext<'_>,
    ) -> AppResult<Option<Action>> {
        match command {
            MainViewCommand::SetMode(mode) => {
                ctx.state.navigation.main_view_mode = *mode;
                ctx.state.navigation.main_view_tab =
                    ctx.state.navigation.main_view_tab.normalize(*mode);
            }
            MainViewCommand::SwitchTab(tab) => {
                ctx.state.navigation.main_view_tab =
                    tab.normalize(ctx.state.navigation.main_view_mode);
            }
            MainViewCommand::NextTab => {
                ctx.state.navigation.next_main_view_tab();
            }
            MainViewCommand::PreviousTab => {
                ctx.state.navigation.previous_main_view_tab();
            }
            MainViewCommand::HydrationStarted | MainViewCommand::HydrationFinished => {}
        }
        Ok(None)
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &AppView<'_>) {
        let is_focused = matches!(ctx.state.navigation.focused_pane, FocusedPane::MainView);
        let mode = ctx.state.navigation.main_view_mode;
        let tab = ctx.state.navigation.main_view_tab.normalize(mode);

        let border_style = if is_focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from("[3] Main View").style(border_style));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(inner);

        if let Some(error) = &ctx.state.search_error {
            let error_widget = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
            frame.render_widget(error_widget, layout[1]);
            return;
        }

        let tab_titles: Vec<Line> = Self::tab_titles(mode)
            .iter()
            .map(|(label, _)| Line::from(*label))
            .collect();
        let tabs = Tabs::new(tab_titles)
            .select(Self::tab_index(mode, tab))
            .highlight_style(Style::default().fg(Color::Cyan));
        frame.render_widget(tabs, layout[0]);

        let selection_text = match (&ctx.state.selected, mode) {
            (Some(entity @ SelectedEntity::Address(addr)), MainViewMode::Address) => {
                let fav_marker = if ctx.state.is_favorite(entity) {
                    " (favorited)"
                } else {
                    ""
                };
                format!(
                    "{} on {}{fav_marker}\nTab: {}\nPress f to toggle favorite",
                    short_hex(&addr.address),
                    addr.chain,
                    Self::tab_titles(mode)[Self::tab_index(mode, tab)].0
                )
            }
            (Some(entity @ SelectedEntity::Transaction(tx)), MainViewMode::Transaction) => {
                let fav_marker = if ctx.state.is_favorite(entity) {
                    " (favorited)"
                } else {
                    ""
                };
                format!(
                    "{} on {}{fav_marker}\nTab: {}\nPress f to toggle favorite",
                    short_hex(&tx.hash),
                    tx.chain,
                    Self::tab_titles(mode)[Self::tab_index(mode, tab)].0
                )
            }
            _ => self.placeholder.clone(),
        };

        let address_data = match (&ctx.state.selected, &ctx.state.current_address) {
            (Some(SelectedEntity::Address(addr)), Some(data))
                if data.identifier == addr.address => Some(data),
            _ => None,
        };
        let transaction_data = match (&ctx.state.selected, &ctx.state.current_transaction) {
            (Some(SelectedEntity::Transaction(tx)), Some(data))
                if data.identifier == tx.hash => Some(data),
            _ => None,
        };

        let tab_summary = if ctx.state.loading.main_view.is_loading {
            "Loading…".to_string()
        } else {
            match mode {
                MainViewMode::Address => {
                    if let Some(data) = address_data {
                        match tab {
                            MainViewTab::AddressTransactions => data.transactions.join("\n"),
                            MainViewTab::AddressInternal => data.internal.join("\n"),
                            MainViewTab::AddressBalances => data.balances.join("\n"),
                            MainViewTab::AddressPermissions => data.permissions.join("\n"),
                            _ => Self::content_for(tab).to_string(),
                        }
                    } else {
                        "No data yet".into()
                    }
                }
                MainViewMode::Transaction => {
                    if let Some(data) = transaction_data {
                        match tab {
                            MainViewTab::TransactionSummary => data.summary.join("\n"),
                            MainViewTab::TransactionDebug => data.debug.join("\n"),
                            MainViewTab::TransactionStorageDiff => data.storage_diff.join("\n"),
                            _ => Self::content_for(tab).to_string(),
                        }
                    } else {
                        "No data yet".into()
                    }
                }
            }
        };

        let body = Paragraph::new(format!("{}\n\n{}", selection_text, tab_summary))
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(body, layout[1]);
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>) -> AppResult<Option<Action>> {
        Ok(None)
    }
}
