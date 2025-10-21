use super::util::short_hex;
use crate::{
    app::{
        Action, AppContext, AppResult, AppView, FocusedPane, HydratedTransaction, MainViewMode,
        MainViewTab, SelectedEntity, TransactionDirection, TransactionRef, TransactionStatus,
    },
    components::Component,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Tabs},
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
    MoveSelectionUp,
    MoveSelectionDown,
    ActivateSelection,
    HydrationStarted,
    HydrationFinished,
}

impl MainView {
    fn tab_titles(mode: MainViewMode) -> &'static [(&'static str, MainViewTab)] {
        match mode {
            MainViewMode::Address => &[
                ("Info", MainViewTab::AddressInfo),
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
            MainViewTab::AddressInfo => "Address overview (placeholder)",
            MainViewTab::TransactionSummary => "Transaction summary (placeholder)",
            MainViewTab::TransactionDebug => "Transaction debugger (placeholder)",
            MainViewTab::TransactionStorageDiff => "Transaction storage diff (placeholder)",
        }
    }

    fn transaction_summary_text(data: &HydratedTransaction) -> String {
        let status = data
            .status
            .map(TransactionStatus::label)
            .unwrap_or("Not cached");
        let from = data
            .from
            .as_ref()
            .map(|addr| short_hex(addr))
            .unwrap_or_else(|| "Not cached".into());
        let to = match (data.to.as_ref(), data.status) {
            (Some(addr), _) => short_hex(addr),
            (None, Some(_)) => "Contract creation".into(),
            (None, None) => "Not cached".into(),
        };
        let value = data
            .value_formatted
            .clone()
            .unwrap_or_else(|| "Not cached".into());
        let block = data
            .block_number
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Not cached".into());
        let calldata_raw = data.calldata.clone();
        let calldata_display = calldata_raw
            .as_ref()
            .map(|value| {
                if value.len() > 66 {
                    format!("{}…", &value[..66])
                } else {
                    value.clone()
                }
            })
            .unwrap_or_else(|| "Not cached".into());

        let mut lines = Vec::new();
        lines.push(format!("Hash: {}", short_hex(&data.identifier)));
        lines.push(format!("Status: {status}"));
        lines.push(format!("From: {from}"));
        lines.push(format!("To: {to}"));
        lines.push(format!("Value: {value}"));
        lines.push(format!("Block: {block}"));
        lines.push(format!("Calldata: {calldata_display}"));

        lines.join("\n")
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
            MainViewCommand::MoveSelectionUp => {
                if ctx.state.navigation.main_view_mode == MainViewMode::Address
                    && !ctx.state.loading.main_view.is_loading
                {
                    let tab = ctx
                        .state
                        .navigation
                        .main_view_tab
                        .normalize(MainViewMode::Address);
                    if matches!(tab, MainViewTab::AddressTransactions) {
                        if let Some(address) = ctx.state.current_address.as_ref() {
                            if let Some(table) = address.transactions_table.as_ref() {
                                ctx.state.address_transactions_view.clamp(table.rows.len());
                                if !table.rows.is_empty() {
                                    if ctx.state.address_transactions_view.selected_index > 0 {
                                        ctx.state.address_transactions_view.selected_index -= 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            MainViewCommand::MoveSelectionDown => {
                if ctx.state.navigation.main_view_mode == MainViewMode::Address
                    && !ctx.state.loading.main_view.is_loading
                {
                    let tab = ctx
                        .state
                        .navigation
                        .main_view_tab
                        .normalize(MainViewMode::Address);
                    if matches!(tab, MainViewTab::AddressTransactions) {
                        if let Some(address) = ctx.state.current_address.as_ref() {
                            if let Some(table) = address.transactions_table.as_ref() {
                                ctx.state.address_transactions_view.clamp(table.rows.len());
                                if !table.rows.is_empty() {
                                    let last = table.rows.len().saturating_sub(1);
                                    let index =
                                        &mut ctx.state.address_transactions_view.selected_index;
                                    if *index < last {
                                        *index += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            MainViewCommand::ActivateSelection => {
                if ctx.state.navigation.main_view_mode == MainViewMode::Address
                    && !ctx.state.loading.main_view.is_loading
                {
                    let tab = ctx
                        .state
                        .navigation
                        .main_view_tab
                        .normalize(MainViewMode::Address);
                    if matches!(tab, MainViewTab::AddressTransactions) {
                        if let (Some(SelectedEntity::Address(addr)), Some(address)) = (
                            ctx.state.selected.as_ref(),
                            ctx.state.current_address.as_ref(),
                        ) {
                            if let Some(table) = address.transactions_table.as_ref() {
                                ctx.state.address_transactions_view.clamp(table.rows.len());
                                if !table.rows.is_empty() {
                                    let index = ctx.state.address_transactions_view.selected_index;
                                    let row = &table.rows[index];
                                    ctx.state.pending_transaction_preview = Some(row.clone());
                                    return Ok(Some(Action::SelectionChanged(
                                        SelectedEntity::Transaction(TransactionRef {
                                            label: short_hex(&row.hash),
                                            hash: row.hash.clone(),
                                            chain: addr.chain.clone(),
                                        }),
                                    )));
                                }
                            }
                        }
                    }
                }
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
                .add_modifier(Modifier::UNDERLINED)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };

        let mode_label = match mode {
            MainViewMode::Address => "Address",
            MainViewMode::Transaction => "Transaction",
        };
        let title = format!("[3] Main View · {mode_label}");

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(title).style(border_style));
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

        let tab_label = Self::tab_titles(mode)[Self::tab_index(mode, tab)].0;
        let selection_text = match (&ctx.state.selected, mode) {
            (Some(entity @ SelectedEntity::Address(addr)), MainViewMode::Address) => {
                let fav_marker = if ctx.state.is_favorite(entity) {
                    " (favorited)"
                } else {
                    ""
                };
                let base = format!(
                    "{} on {}{fav_marker}\nTab: {}",
                    short_hex(&addr.address),
                    addr.chain,
                    tab_label
                );
                if matches!(tab, MainViewTab::AddressTransactions) {
                    format!("{base}\n[Enter] Open transaction • [F] Favorite/Remove")
                } else {
                    format!("{base}\n[F] Favorite/Remove")
                }
            }
            (Some(entity @ SelectedEntity::Transaction(tx)), MainViewMode::Transaction) => {
                let fav_marker = if ctx.state.is_favorite(entity) {
                    " (favorited)"
                } else {
                    ""
                };
                format!(
                    "{} on {}{fav_marker}\nTab: {}\n[F] Favorite/Remove",
                    short_hex(&tx.hash),
                    tx.chain,
                    tab_label
                )
            }
            _ => self.placeholder.clone(),
        };

        let address_data = match (&ctx.state.selected, &ctx.state.current_address) {
            (Some(SelectedEntity::Address(addr)), Some(data))
                if data.identifier == addr.address =>
            {
                Some(data)
            }
            _ => None,
        };
        let transaction_data = match (&ctx.state.selected, &ctx.state.current_transaction) {
            (Some(SelectedEntity::Transaction(tx)), Some(data)) if data.identifier == tx.hash => {
                Some(data)
            }
            _ => None,
        };

        let tab_summary = if ctx.state.loading.main_view.is_loading {
            "Loading…".to_string()
        } else {
            match mode {
                MainViewMode::Address => {
                    if let Some(data) = address_data {
                        match tab {
                            MainViewTab::AddressInfo => data.info.join("\n"),
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
                            MainViewTab::TransactionSummary => Self::transaction_summary_text(data),
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

        let summary_content = if tab_summary.trim().is_empty() {
            selection_text.clone()
        } else {
            format!("{selection_text}\n\n{tab_summary}")
        };

        if mode == MainViewMode::Address
            && matches!(tab, MainViewTab::AddressTransactions)
            && !ctx.state.loading.main_view.is_loading
        {
            if let Some(address) = address_data {
                if let Some(table) = address.transactions_table.as_ref() {
                    if !table.rows.is_empty() && layout[1].height >= 4 {
                        let available_height = layout[1].height;
                        let mut summary_height = summary_content.lines().count() as u16;
                        if summary_height == 0 {
                            summary_height = 1;
                        }
                        summary_height =
                            summary_height.min(available_height.saturating_sub(2).max(2));

                        let content_chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([Constraint::Length(summary_height), Constraint::Min(2)])
                            .split(layout[1]);

                        let summary_widget = Paragraph::new(summary_content.clone())
                            .style(Style::default().fg(Color::Gray));
                        frame.render_widget(summary_widget, content_chunks[0]);

                        let rows: Vec<Row<'_>> = table
                            .rows
                            .iter()
                            .map(|row| {
                                let status_style = match row.status {
                                    TransactionStatus::Failed => {
                                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                                    }
                                    TransactionStatus::Success => Style::default().fg(Color::Green),
                                };
                                let direction_style = match row.direction {
                                    TransactionDirection::Incoming => {
                                        Style::default().fg(Color::Green)
                                    }
                                    TransactionDirection::Outgoing => {
                                        Style::default().fg(Color::Red)
                                    }
                                    TransactionDirection::SelfTransfer => {
                                        Style::default().fg(Color::Yellow)
                                    }
                                    TransactionDirection::Interaction => Style::default(),
                                };
                                let value_style = match row.direction {
                                    TransactionDirection::Incoming => {
                                        Style::default().fg(Color::Green)
                                    }
                                    TransactionDirection::Outgoing => {
                                        Style::default().fg(Color::Red)
                                    }
                                    _ => Style::default(),
                                };
                                let status_cell =
                                    Cell::from(row.status.label()).style(status_style);
                                let hash_cell = Cell::from(short_hex(&row.hash));
                                let direction_cell =
                                    Cell::from(row.direction.label()).style(direction_style);
                                let spacer_cell = Cell::from("");
                                let counterparty_cell = Cell::from(row.counterparty.as_str());
                                let value_cell =
                                    Cell::from(row.value_display.as_str()).style(value_style);
                                let block_cell = Cell::from(
                                    row.block_number
                                        .map(|n| n.to_string())
                                        .unwrap_or_else(|| "?".into()),
                                );
                                Row::new(vec![
                                    status_cell,
                                    hash_cell,
                                    direction_cell,
                                    spacer_cell,
                                    counterparty_cell,
                                    value_cell,
                                    block_cell,
                                ])
                            })
                            .collect();

                        let header = Row::new(vec![
                            "Status",
                            "Tx Hash",
                            "Direction",
                            "",
                            "Counterparty",
                            "Value",
                            "Block",
                        ])
                        .style(Style::default().add_modifier(Modifier::BOLD));

                        let mut state = TableState::default();
                        let selected = ctx
                            .state
                            .address_transactions_view
                            .selected_index
                            .min(table.rows.len().saturating_sub(1));
                        state.select(Some(selected));

                        let widths = [
                            Constraint::Length(7),
                            Constraint::Length(14),
                            Constraint::Length(11),
                            Constraint::Length(2),
                            Constraint::Fill(1),
                            Constraint::Length(15),
                            Constraint::Length(8),
                        ];

                        let table_widget = Table::new(rows, widths)
                            .header(header)
                            .column_spacing(1)
                            .highlight_symbol("▸ ")
                            .row_highlight_style(
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            );

                        frame.render_stateful_widget(table_widget, content_chunks[1], &mut state);
                        return;
                    }
                }
            }
        }

        let body = Paragraph::new(summary_content).style(Style::default().fg(Color::Gray));
        frame.render_widget(body, layout[1]);
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>) -> AppResult<Option<Action>> {
        Ok(None)
    }
}
