use crate::{
    components::Component,
    storage::{FavoriteRecord, SecretKey, SecretsRepository, Storage},
    ui::util::short_hex,
    ui::{
        bottom_bar::BottomBar,
        main_view::{MainView, MainViewCommand},
        modal::{SecretsModal, secrets::SecretsFormCommand},
        sidebar::{Sidebar, SidebarCommand},
        top::{TopBar, TopCommand},
    },
};
pub type AppResult<T> = color_eyre::Result<T>;
use alloy::primitives::{Address, U256, utils::format_units};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
};
use std::{
    collections::{HashMap, HashSet},
    env,
    sync::mpsc,
    time::{Duration as StdDuration, Instant},
};

use tokio::runtime::{Handle, Runtime};
use tokio::time::{Duration, sleep, timeout};

pub use navigation::{FocusedPane, MainViewMode, MainViewTab, SidebarTab};

mod anvil;
use self::anvil::{AccountOverview, fetch_account_overview, fetch_latest_block};
mod etherscan;
use self::etherscan::{AddressTransaction, TransactionFetchError, fetch_address_transactions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedEntity {
    Address(AddressRef),
    Transaction(TransactionRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressRef {
    pub label: String,
    pub address: String,
    pub chain: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRef {
    pub label: String,
    pub hash: String,
    pub chain: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydratedAddress {
    pub identifier: String,
    pub info: Vec<String>,
    pub transactions: Vec<String>,
    pub transactions_table: Option<AddressTransactionsTable>,
    pub internal: Vec<String>,
    pub balances: Vec<String>,
    pub permissions: Vec<String>,
    pub overview: Option<AccountOverview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressTransactionsTable {
    pub source_label: String,
    pub source_api_version: String,
    pub limit: usize,
    pub rows: Vec<AddressTransactionRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressTransactionRow {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value_wei: U256,
    pub block_number: Option<u64>,
    pub direction: TransactionDirection,
    pub counterparty: String,
    pub value_display: String,
    pub status: TransactionStatus,
    pub calldata: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionDirection {
    Incoming,
    Outgoing,
    SelfTransfer,
    Interaction,
}

impl TransactionStatus {
    pub fn label(self) -> &'static str {
        match self {
            TransactionStatus::Success => "OK",
            TransactionStatus::Failed => "Failed",
        }
    }
}

impl TransactionDirection {
    pub fn label(self) -> &'static str {
        match self {
            TransactionDirection::Incoming => "Incoming",
            TransactionDirection::Outgoing => "Outgoing",
            TransactionDirection::SelfTransfer => "Self",
            TransactionDirection::Interaction => "Interaction",
        }
    }
}

impl AddressTransactionRow {
    pub fn from_transaction(target_address: &str, tx: &AddressTransaction) -> Self {
        let is_sender = tx.from.eq_ignore_ascii_case(target_address);
        let is_recipient = tx
            .to
            .as_ref()
            .map(|addr| addr.eq_ignore_ascii_case(target_address))
            .unwrap_or(false);

        let direction = if is_sender && is_recipient {
            TransactionDirection::SelfTransfer
        } else if is_sender {
            TransactionDirection::Outgoing
        } else if is_recipient {
            TransactionDirection::Incoming
        } else {
            TransactionDirection::Interaction
        };

        let counterparty = if is_sender && is_recipient {
            "Self".to_string()
        } else if is_sender {
            tx.to
                .as_ref()
                .map(|addr| short_hex(addr))
                .unwrap_or_else(|| "Contract creation".into())
        } else if is_recipient {
            short_hex(&tx.from)
        } else {
            tx.to
                .as_ref()
                .map(|addr| short_hex(addr))
                .unwrap_or_else(|| short_hex(&tx.from))
        };

        let mut value = format_eth_value(&tx.value_wei);
        if !tx.value_wei.is_zero() {
            match direction {
                TransactionDirection::Outgoing => value = format!("-{value}"),
                TransactionDirection::Incoming => value = format!("+{value}"),
                _ => {}
            }
        }

        AddressTransactionRow {
            hash: tx.hash.clone(),
            from: tx.from.clone(),
            to: tx.to.clone(),
            value_wei: tx.value_wei,
            block_number: (tx.block_number > 0).then_some(tx.block_number),
            direction,
            counterparty,
            value_display: value,
            status: if tx.is_error {
                TransactionStatus::Failed
            } else {
                TransactionStatus::Success
            },
            calldata: tx.input.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydratedTransaction {
    pub identifier: String,
    pub summary: Vec<String>,
    pub debug: Vec<String>,
    pub storage_diff: Vec<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub value_formatted: Option<String>,
    pub calldata: Option<String>,
    pub block_number: Option<u64>,
    pub status: Option<TransactionStatus>,
}

#[derive(Debug, Clone, Default)]
pub struct SecretsState {
    pub etherscan_api_key: Option<String>,
    pub anvil_rpc_url: Option<String>,
}

impl SecretsState {
    fn load(storage: &Storage) -> AppResult<Self> {
        let repo = storage.secrets();
        Ok(Self {
            etherscan_api_key: Self::resolve_secret(repo, SecretKey::EtherscanApiKey)?,
            anvil_rpc_url: Self::resolve_secret(repo, SecretKey::AnvilRpcUrl)?,
        })
    }

    fn resolve_secret(repo: &SecretsRepository, key: SecretKey) -> AppResult<Option<String>> {
        if let Ok(value) = env::var(key.env_var()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                repo.set(key, trimmed)?;
                return Ok(Some(trimmed.to_string()));
            }
            repo.remove(key)?;
            return Ok(None);
        }
        let stored = repo.get(key)?;
        Ok(stored.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }))
    }
}

/// Central application type that orchestrates state and delegates to UI components.
pub struct App {
    running: bool,
    pub state: AppState,
    pub storage: Storage,
    top_bar: TopBar,
    sidebar: Sidebar,
    main_view: MainView,
    bottom_bar: BottomBar,
    runtime: Runtime,
    message_rx: mpsc::Receiver<Message>,
    message_tx: mpsc::Sender<Message>,
    secrets_modal: Option<SecretsModal>,
}

impl App {
    pub fn new() -> AppResult<Self> {
        let mut state = AppState::default();
        let mut storage = Storage::open_default()?;
        state.secrets = SecretsState::load(&storage)?;
        let mut top_bar = TopBar::default();
        let mut sidebar = Sidebar::default();
        let mut main_view = MainView::default();
        let mut bottom_bar = BottomBar::default();
        let runtime = Runtime::new()?;
        let runtime_handle = runtime.handle().clone();
        let (message_tx, message_rx) = mpsc::channel();

        {
            let mut ctx = AppContext {
                state: &mut state,
                storage: &mut storage,
                commands: CommandBus::new(message_tx.clone(), runtime_handle.clone()),
            };
            top_bar.init(&mut ctx)?;
            sidebar.init(&mut ctx)?;
            main_view.init(&mut ctx)?;
            bottom_bar.init(&mut ctx)?;
        }

        let mut secrets_modal = None;
        if state.secrets.etherscan_api_key.is_none() || state.secrets.anvil_rpc_url.is_none() {
            let mut modal = SecretsModal::new();
            {
                let mut ctx = AppContext {
                    state: &mut state,
                    storage: &mut storage,
                    commands: CommandBus::new(message_tx.clone(), runtime_handle.clone()),
                };
                modal.init(&mut ctx)?;
            }
            state.navigation.focus_modal();
            secrets_modal = Some(modal);
        }

        // Hydrate favorites from storage
        let address_records = storage.favorites_addresses().list()?;
        let mut address_refs = Vec::new();
        for record in address_records {
            state.favorite_addresses.insert(record.identifier.clone());
            address_refs.push(AddressRef {
                label: record
                    .label
                    .clone()
                    .unwrap_or_else(|| record.identifier.clone()),
                address: record.identifier,
                chain: record.chain,
            });
        }
        sidebar.set_addresses(address_refs, state.navigation.sidebar_tab);

        let transaction_records = storage.favorites_transactions().list()?;
        let mut transaction_refs = Vec::new();
        for record in transaction_records {
            state
                .favorite_transactions
                .insert(record.identifier.clone());
            transaction_refs.push(TransactionRef {
                label: record
                    .label
                    .clone()
                    .unwrap_or_else(|| record.identifier.clone()),
                hash: record.identifier,
                chain: record.chain,
            });
        }
        sidebar.set_transactions(transaction_refs, state.navigation.sidebar_tab);

        state.selected = sidebar
            .current_selection(state.navigation.sidebar_tab, 0)
            .or_else(|| match state.navigation.sidebar_tab {
                SidebarTab::Addresses => sidebar.current_selection(SidebarTab::Transactions, 0),
                SidebarTab::Transactions => sidebar.current_selection(SidebarTab::Addresses, 0),
            });
        if let Some(entity) = state.selected.clone() {
            match entity {
                SelectedEntity::Address(_) => {
                    state.navigation.main_view_mode = MainViewMode::Address;
                    state.navigation.main_view_tab = MainViewTab::AddressTransactions;
                }
                SelectedEntity::Transaction(_) => {
                    state.navigation.main_view_mode = MainViewMode::Transaction;
                    state.navigation.main_view_tab = MainViewTab::TransactionSummary;
                }
            }
        }

        let mut app = Self {
            running: false,
            state,
            storage,
            top_bar,
            sidebar,
            main_view,
            bottom_bar,
            runtime,
            message_rx,
            message_tx: message_tx.clone(),
            secrets_modal,
        };

        if let Some(entity) = app.state.selected.clone() {
            app.start_hydration(entity);
        }

        Ok(app)
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> AppResult<()> {
        self.running = true;
        while self.running {
            self.tick()?;
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(frame.area());

        let top_area = layout[0];
        let main_area = layout[1];
        let bottom_area = layout[2];

        let app_panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(32), Constraint::Min(1)])
            .split(main_area);

        let sidebar_area = app_panes[0];
        let content_area = app_panes[1];

        let view = AppView { state: &self.state };

        self.top_bar.render(frame, top_area, &view);
        self.sidebar.render(frame, sidebar_area, &view);
        self.main_view.render(frame, content_area, &view);
        self.bottom_bar.render(frame, bottom_area, &view);

        if let Some(modal) = self.secrets_modal.as_mut() {
            let area = frame.area();
            modal.render(frame, area, &view);
        }
    }

    fn handle_events(&mut self) -> AppResult<()> {
        if event::poll(StdDuration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key_event(key)?,
                Event::Paste(content) => self.on_paste_event(content)?,
                Event::Mouse(_) | Event::Resize(_, _) => {}
                _ => {}
            }
        }
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) -> AppResult<()> {
        if matches!(self.state.navigation.focused_pane, FocusedPane::Modal) {
            self.handle_modal_key(key)?;
            return Ok(());
        }

        if self.top_bar.is_search_active() {
            match key.code {
                KeyCode::Esc => {
                    self.top_bar_command(TopCommand::Cancel)?;
                    return Ok(());
                }
                KeyCode::Enter => {
                    self.top_bar_command(TopCommand::Submit)?;
                    return Ok(());
                }
                KeyCode::Backspace => {
                    self.top_bar_command(TopCommand::Backspace)?;
                    return Ok(());
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.top_bar_command(TopCommand::InputChar(c))?;
                    return Ok(());
                }
                _ => {}
            }
        }

        match (key.modifiers, key.code) {
            (_, KeyCode::Esc | KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => {
                self.dispatch(Action::Quit)
            }
            (KeyModifiers::NONE, KeyCode::Char('/')) => {
                self.dispatch(Action::FocusPane(FocusedPane::Top));
                self.top_bar_command(TopCommand::ActivateSearch)?;
            }
            (KeyModifiers::NONE, KeyCode::Tab) => self.dispatch(Action::FocusNextPane),
            (KeyModifiers::SHIFT, KeyCode::Tab) => self.dispatch(Action::FocusPreviousPane),
            (KeyModifiers::NONE, KeyCode::Char('[')) => {
                self.handle_tab_navigation(TabDirection::Previous)?;
            }
            (KeyModifiers::NONE, KeyCode::Char(']')) => {
                self.handle_tab_navigation(TabDirection::Next)?;
            }
            (KeyModifiers::NONE, KeyCode::Char('h')) => {
                self.handle_movement(Movement::Left)?;
            }
            (KeyModifiers::NONE, KeyCode::Char('j')) => {
                self.handle_movement(Movement::Down)?;
            }
            (KeyModifiers::NONE, KeyCode::Char('k')) => {
                self.handle_movement(Movement::Up)?;
            }
            (KeyModifiers::NONE, KeyCode::Char('l')) => {
                self.handle_movement(Movement::Right)?;
            }
            (KeyModifiers::NONE, KeyCode::Char(d)) if d.is_ascii_digit() => {
                if let Some(pane) = d
                    .to_digit(10)
                    .and_then(|n| FocusedPane::from_number(n as usize))
                {
                    self.dispatch(Action::FocusPane(pane));
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => match self.state.navigation.focused_pane {
                FocusedPane::MainView => {
                    self.main_view_command(MainViewCommand::ActivateSelection)?;
                }
                FocusedPane::Sidebar => {
                    if let Some(entity) = self
                        .sidebar
                        .active_selection(self.state.navigation.sidebar_tab)
                    {
                        self.dispatch(Action::SelectionChanged(entity));
                    }
                    self.dispatch(Action::FocusPane(FocusedPane::MainView));
                }
                _ => {}
            },
            (KeyModifiers::NONE, KeyCode::Char('f'))
                if matches!(self.state.navigation.focused_pane, FocusedPane::MainView) =>
            {
                self.toggle_favorite()?;
            }
            (KeyModifiers::SHIFT, KeyCode::Char('F'))
                if matches!(self.state.navigation.focused_pane, FocusedPane::MainView) =>
            {
                self.toggle_favorite()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_paste_event(&mut self, content: String) -> AppResult<()> {
        if matches!(self.state.navigation.focused_pane, FocusedPane::Modal) {
            self.handle_modal_paste(content)?;
        } else if self.top_bar.is_search_active() {
            self.handle_search_paste(content)?;
        }
        Ok(())
    }

    fn handle_modal_key(&mut self, key: KeyEvent) -> AppResult<()> {
        use crossterm::event::KeyCode;

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            self.dispatch(Action::Quit);
            return Ok(());
        }

        if let Some(command) = SecretsModal::command_from_key(key) {
            let commands = self.command_bus();
            let action = if let Some(modal) = self.secrets_modal.as_mut() {
                let mut ctx = AppContext {
                    state: &mut self.state,
                    storage: &mut self.storage,
                    commands,
                };
                modal.update(&command, &mut ctx)?
            } else {
                None
            };
            if let Some(action) = action {
                self.dispatch(action);
            }
        }
        Ok(())
    }

    fn handle_modal_paste(&mut self, content: String) -> AppResult<()> {
        if self.secrets_modal.is_some() {
            let commands = self.command_bus();
            let action = if let Some(modal) = self.secrets_modal.as_mut() {
                let mut ctx = AppContext {
                    state: &mut self.state,
                    storage: &mut self.storage,
                    commands,
                };
                modal.update(&SecretsFormCommand::InsertText(content), &mut ctx)?
            } else {
                None
            };
            if let Some(action) = action {
                self.dispatch(action);
            }
        }
        Ok(())
    }

    fn handle_search_paste(&mut self, content: String) -> AppResult<()> {
        for ch in content.chars() {
            if matches!(ch, '\r' | '\n') {
                continue;
            }
            self.top_bar_command(TopCommand::InputChar(ch))?;
        }
        Ok(())
    }

    async fn hydrate_address(addr: AddressRef, secrets: SecretsState) -> HydratedAddress {
        const TRANSACTION_FETCH_LIMIT: usize = 25;
        let mut rpc_url = secrets.anvil_rpc_url.clone();
        if rpc_url.is_none() {
            if let Ok(env_url) = std::env::var("ANVIL_RPC_URL") {
                if !env_url.trim().is_empty() {
                    rpc_url = Some(env_url);
                }
            }
        }

        let mut overview: Option<AccountOverview> = None;
        let mut note: Option<String> = None;
        let mut block_note: Option<String> = None;

        if let Some(rpc_value) = rpc_url.clone() {
            match addr.address.parse::<Address>() {
                Ok(parsed) => {
                    match timeout(
                        Duration::from_secs(10),
                        fetch_account_overview(&rpc_value, parsed),
                    )
                    .await
                    {
                        Ok(Ok(data)) => {
                            block_note = None;
                            overview = Some(data);
                        }
                        Ok(Err(error)) => {
                            note = Some(format!("Failed to load account data: {error}"));
                            if let Ok(result) =
                                timeout(Duration::from_secs(4), fetch_latest_block(&rpc_value))
                                    .await
                            {
                                if let Ok(block) = result {
                                    block_note = Some(format!("Latest block observed: {block}"));
                                }
                            }
                        }
                        Err(_) => {
                            note = Some(format!("Account query to {rpc_value} timed out"));
                            if let Ok(result) =
                                timeout(Duration::from_secs(4), fetch_latest_block(&rpc_value))
                                    .await
                            {
                                if let Ok(block) = result {
                                    block_note = Some(format!("Latest block observed: {block}"));
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    note = Some("Address is not a valid hexadecimal string".into());
                }
            }
        } else {
            note = Some("Configure an Anvil RPC endpoint to load account data.".into());
        }

        let transactions_result = fetch_address_transactions(
            &addr,
            secrets.etherscan_api_key.as_deref(),
            TRANSACTION_FETCH_LIMIT,
        )
        .await;

        let mut hydrated = build_address_view(addr, overview, note, rpc_url, block_note);

        match transactions_result {
            Ok((entries, source)) => {
                let rows: Vec<AddressTransactionRow> = entries
                    .iter()
                    .map(|tx| AddressTransactionRow::from_transaction(&hydrated.identifier, tx))
                    .collect();
                if rows.is_empty() {
                    hydrated.transactions = vec![format!(
                        "No transactions available via {} ({}).",
                        source.label, source.api_version
                    )];
                    hydrated.transactions_table = None;
                } else {
                    hydrated.transactions = vec![format!(
                        "Latest {} transaction(s) via {} ({}) • newest first (max {}).",
                        rows.len(),
                        source.label,
                        source.api_version,
                        TRANSACTION_FETCH_LIMIT
                    )];
                    hydrated.transactions_table = Some(AddressTransactionsTable {
                        source_label: source.label.into(),
                        source_api_version: source.api_version.into(),
                        limit: TRANSACTION_FETCH_LIMIT,
                        rows,
                    });
                }
            }
            Err(TransactionFetchError::MissingApiKey) => {
                hydrated.transactions = vec![
                    "Add an Etherscan API key to load recent transactions.".into(),
                    "Open Settings → Secrets and enter ETHERSCAN_API_KEY.".into(),
                ];
                hydrated.transactions_table = None;
            }
            Err(TransactionFetchError::UnsupportedChain(chain)) => {
                hydrated.transactions = vec![format!(
                    "No Etherscan-compatible explorer configured for chain {chain}."
                )];
                hydrated.transactions_table = None;
            }
            Err(err) => {
                hydrated.transactions = vec![format!("Failed to load transactions: {err}")];
                hydrated.transactions_table = None;
            }
        };

        hydrated
    }

    #[cfg(test)]
    fn secrets_modal_mut(&mut self) -> Option<&mut SecretsModal> {
        self.secrets_modal.as_mut()
    }

    fn dispatch(&mut self, action: Action) {
        match action {
            Action::Quit => self.running = false,
            Action::FocusPane(pane) => self.state.navigation.focus_pane(pane),
            Action::FocusNextPane => self.state.navigation.focus_next(),
            Action::FocusPreviousPane => self.state.navigation.focus_previous(),
            Action::SelectionChanged(entity) => {
                self.state.selected = Some(entity.clone());
                self.state.search_error = None;
                match entity {
                    SelectedEntity::Address(_) => {
                        self.state.address_transactions_view.reset();
                        self.state.navigation.main_view_mode = MainViewMode::Address;
                        self.state.navigation.main_view_tab = MainViewTab::AddressInfo;
                    }
                    SelectedEntity::Transaction(_) => {
                        self.state.navigation.main_view_mode = MainViewMode::Transaction;
                        self.state.navigation.main_view_tab = MainViewTab::TransactionSummary;
                    }
                }
                self.start_hydration(entity);
            }
            Action::LoadingStarted(pane) => self.state.loading.set_loading(pane, true),
            Action::LoadingFinished(pane) => self.state.loading.set_loading(pane, false),
            Action::CloseModal => self.close_modal(),
            Action::SecretsSaved => {
                self.close_modal();
                self.show_status("Secrets updated");
            }
        }
    }

    fn handle_tab_navigation(&mut self, direction: TabDirection) -> AppResult<()> {
        match self.state.navigation.focused_pane {
            FocusedPane::Sidebar => match direction {
                TabDirection::Next => self.sidebar_command(SidebarCommand::NextTab)?,
                TabDirection::Previous => self.sidebar_command(SidebarCommand::PreviousTab)?,
            },
            FocusedPane::MainView => match direction {
                TabDirection::Next => self.main_view_command(MainViewCommand::NextTab)?,
                TabDirection::Previous => self.main_view_command(MainViewCommand::PreviousTab)?,
            },
            _ => {}
        }
        Ok(())
    }

    fn handle_movement(&mut self, movement: Movement) -> AppResult<()> {
        match self.state.navigation.focused_pane {
            FocusedPane::Sidebar => match movement {
                Movement::Up => self.sidebar_command(SidebarCommand::MoveUp)?,
                Movement::Down => self.sidebar_command(SidebarCommand::MoveDown)?,
                Movement::Left | Movement::Right => {}
            },
            FocusedPane::MainView => match movement {
                Movement::Up => self.main_view_command(MainViewCommand::MoveSelectionUp)?,
                Movement::Down => self.main_view_command(MainViewCommand::MoveSelectionDown)?,
                Movement::Left | Movement::Right => {}
            },
            FocusedPane::Top | FocusedPane::BottomBar | FocusedPane::Modal => {}
        }
        Ok(())
    }

    fn sidebar_command(&mut self, command: SidebarCommand) -> AppResult<()> {
        let commands = self.command_bus();
        let mut ctx = AppContext {
            state: &mut self.state,
            storage: &mut self.storage,
            commands,
        };
        if let Some(action) = self.sidebar.update(&command, &mut ctx)? {
            self.dispatch(action);
        }
        Ok(())
    }

    fn main_view_command(&mut self, command: MainViewCommand) -> AppResult<()> {
        let commands = self.command_bus();
        let mut ctx = AppContext {
            state: &mut self.state,
            storage: &mut self.storage,
            commands,
        };
        if let Some(action) = self.main_view.update(&command, &mut ctx)? {
            self.dispatch(action);
        }
        Ok(())
    }

    fn top_bar_command(&mut self, command: TopCommand) -> AppResult<()> {
        let commands = self.command_bus();
        let mut ctx = AppContext {
            state: &mut self.state,
            storage: &mut self.storage,
            commands,
        };
        if let Some(action) = self.top_bar.update(&command, &mut ctx)? {
            self.dispatch(action);
        }
        Ok(())
    }

    fn command_bus(&self) -> CommandBus {
        let handle = self.runtime.handle().clone();
        CommandBus::new(self.message_tx.clone(), handle)
    }

    fn close_modal(&mut self) {
        self.secrets_modal = None;
        self.state.navigation.restore_focus_after_modal();
    }

    fn show_status(&mut self, message: impl Into<String>) {
        if let Err(err) = self.top_bar_command(TopCommand::ShowStatus(message.into())) {
            eprintln!("failed to update status: {err:?}");
        }
    }

    fn start_hydration(&mut self, entity: SelectedEntity) {
        match entity {
            SelectedEntity::Address(addr) => self.start_address_hydration(addr),
            SelectedEntity::Transaction(tx) => {
                let mut preview = self.state.pending_transaction_preview.take();
                if preview.is_none() {
                    preview = self.state.transaction_preview_cache.get(&tx.hash).cloned();
                }
                self.start_transaction_hydration(tx, preview);
            }
        }
    }

    fn start_address_hydration(&mut self, addr: AddressRef) {
        self.state.current_address = None;
        self.state.loading.set_loading(FocusedPane::MainView, true);
        self.show_status(format!(
            "Fetching latest activity for {}",
            short_hex(&addr.address)
        ));
        let bus = self.command_bus();
        let secrets = self.state.secrets.clone();
        bus.spawn_async(move || {
            let addr_ref = addr.clone();
            let secrets_clone = secrets.clone();
            async move {
                let data = Self::hydrate_address(addr_ref.clone(), secrets_clone).await;
                Message::AddressHydrated(data)
            }
        });
    }

    fn start_transaction_hydration(
        &mut self,
        tx: TransactionRef,
        preview: Option<AddressTransactionRow>,
    ) {
        self.state.current_transaction = None;
        self.state.loading.set_loading(FocusedPane::MainView, true);
        self.show_status(format!("Loading transaction {}", short_hex(&tx.hash)));
        if let Some(row) = preview.as_ref() {
            self.state
                .transaction_preview_cache
                .insert(row.hash.clone(), row.clone());
        }
        let bus = self.command_bus();
        bus.spawn_async(move || {
            let tx_ref = tx.clone();
            let preview_clone = preview.clone();
            async move {
                sleep(Duration::from_millis(350)).await;
                let short = short_hex(&tx_ref.hash);
                let mut summary = vec![format!("Hash: {}", short)];
                let mut status = None;
                let mut block_number = None;
                let mut from = None;
                let mut to = None;
                let mut value_formatted = None;
                let preview_calldata = preview_clone.as_ref().and_then(|row| row.calldata.clone());
                let calldata_message = preview_calldata.clone().unwrap_or_else(|| {
                    "Calldata unavailable (connect debugger or provider)".to_string()
                });
                if let Some(row) = preview_clone.as_ref() {
                    from = Some(row.from.clone());
                    to = row.to.clone();
                    value_formatted = Some(row.value_display.clone());
                    block_number = row.block_number;
                    status = Some(row.status);
                    summary.push(format!("Status: {}", row.status.label()));
                    summary.push(format!("From: {}", short_hex(&row.from)));
                    summary.push(format!(
                        "To: {}",
                        row.to
                            .as_ref()
                            .map(|addr| short_hex(addr))
                            .unwrap_or_else(|| "Contract creation".into())
                    ));
                    summary.push(format!("Value: {}", row.value_display));
                    if let Some(block) = row.block_number {
                        summary.push(format!("Block: {block}"));
                    }
                } else {
                    summary.push("Status: Not cached".into());
                    summary.push("From: Not cached".into());
                    summary.push("To: Not cached".into());
                    summary.push("Value: Not cached".into());
                }
                summary.push(format!("Calldata: {calldata_message}"));
                Message::TransactionHydrated(HydratedTransaction {
                    identifier: tx_ref.hash.clone(),
                    summary,
                    debug: vec!["Trace data unavailable. Configure Alloy debug adapter.".into()],
                    storage_diff: vec!["Storage diff requires debugger export (`e`).".into()],
                    from,
                    to,
                    value_formatted,
                    calldata: preview_calldata,
                    block_number,
                    status,
                })
            }
        });
    }

    fn toggle_favorite(&mut self) -> AppResult<()> {
        if let Some(selected) = self.state.selected.clone() {
            match &selected {
                SelectedEntity::Address(addr) => {
                    let key = addr.address.clone();
                    if self.state.favorite_addresses.contains(&key) {
                        self.storage.favorites_addresses().remove(&key)?;
                        self.state.favorite_addresses.remove(&key);
                        self.sidebar_command(SidebarCommand::RemoveFavorite(selected.clone()))?;
                        self.top_bar_command(TopCommand::ShowStatus(format!(
                            "Removed {} from favorites",
                            short_hex(&addr.address)
                        )))?;
                    } else {
                        let record = FavoriteRecord {
                            label: Some(addr.label.clone()),
                            identifier: addr.address.clone(),
                            chain: addr.chain.clone(),
                        };
                        self.storage.favorites_addresses().upsert(&record)?;
                        self.state.favorite_addresses.insert(key);
                        self.sidebar_command(SidebarCommand::AddFavorite(selected.clone()))?;
                        self.top_bar_command(TopCommand::ShowStatus(format!(
                            "Favorited {}",
                            short_hex(&addr.address)
                        )))?;
                    }
                }
                SelectedEntity::Transaction(tx) => {
                    let key = tx.hash.clone();
                    if self.state.favorite_transactions.contains(&key) {
                        self.storage.favorites_transactions().remove(&key)?;
                        self.state.favorite_transactions.remove(&key);
                        self.sidebar_command(SidebarCommand::RemoveFavorite(selected.clone()))?;
                        self.top_bar_command(TopCommand::ShowStatus(format!(
                            "Removed {} from favorites",
                            short_hex(&tx.hash)
                        )))?;
                    } else {
                        let record = FavoriteRecord {
                            label: Some(tx.label.clone()),
                            identifier: tx.hash.clone(),
                            chain: tx.chain.clone(),
                        };
                        self.storage.favorites_transactions().upsert(&record)?;
                        self.state.favorite_transactions.insert(key);
                        self.sidebar_command(SidebarCommand::AddFavorite(selected.clone()))?;
                        self.top_bar_command(TopCommand::ShowStatus(format!(
                            "Favorited {}",
                            short_hex(&tx.hash)
                        )))?;
                    }
                }
            }
        }
        Ok(())
    }

    fn tick(&mut self) -> AppResult<()> {
        {
            let commands = self.command_bus();
            let (state, storage) = (&mut self.state, &mut self.storage);
            let mut ctx = AppContext {
                state,
                storage,
                commands,
            };
            if let Some(action) = self.top_bar.tick(&mut ctx)? {
                self.dispatch(action);
            }
        }
        {
            let commands = self.command_bus();
            let (state, storage) = (&mut self.state, &mut self.storage);
            let mut ctx = AppContext {
                state,
                storage,
                commands,
            };
            if let Some(action) = self.sidebar.tick(&mut ctx)? {
                self.dispatch(action);
            }
        }
        {
            let commands = self.command_bus();
            let (state, storage) = (&mut self.state, &mut self.storage);
            let mut ctx = AppContext {
                state,
                storage,
                commands,
            };
            if let Some(action) = self.main_view.tick(&mut ctx)? {
                self.dispatch(action);
            }
        }
        {
            let commands = self.command_bus();
            let (state, storage) = (&mut self.state, &mut self.storage);
            let mut ctx = AppContext {
                state,
                storage,
                commands,
            };
            if let Some(action) = self.bottom_bar.tick(&mut ctx)? {
                self.dispatch(action);
            }
        }
        if self.secrets_modal.is_some() {
            let commands = self.command_bus();
            let action = if let Some(modal) = self.secrets_modal.as_mut() {
                let mut ctx = AppContext {
                    state: &mut self.state,
                    storage: &mut self.storage,
                    commands,
                };
                modal.tick(&mut ctx)?
            } else {
                None
            };
            if let Some(action) = action {
                self.dispatch(action);
            }
        }
        self.drain_messages();
        Ok(())
    }
    fn drain_messages(&mut self) {
        while let Ok(message) = self.message_rx.try_recv() {
            match message {
                Message::SearchCompleted { query, entity } => {
                    let _ = self.top_bar_command(TopCommand::SearchCompleted {
                        query: query.clone(),
                        entity: entity.clone(),
                    });
                    self.dispatch(Action::LoadingFinished(FocusedPane::Top));
                    self.dispatch(Action::SelectionChanged(entity));
                    self.dispatch(Action::FocusPane(FocusedPane::MainView));
                }
                Message::SearchFailed { query, error } => {
                    let _ = self.top_bar_command(TopCommand::SearchFailed {
                        query: query.clone(),
                        error: error.clone(),
                    });
                    self.dispatch(Action::LoadingFinished(FocusedPane::Top));
                    self.state.search_error = Some(error.clone());
                    eprintln!("search error: {error}");
                }
                Message::AddressHydrated(data) => {
                    if let Some(SelectedEntity::Address(addr)) = self.state.selected.as_ref() {
                        if addr.address == data.identifier {
                            let cached_rows = data
                                .transactions_table
                                .as_ref()
                                .map(|table| table.rows.clone());
                            let status_message = data
                                .overview
                                .as_ref()
                                .and_then(|ov| {
                                    format_units(ov.balance_wei, "ether")
                                        .ok()
                                        .map(|balance| format!("Balance: {balance} ETH"))
                                })
                                .or_else(|| {
                                    data.info
                                        .iter()
                                        .find(|line| {
                                            line.contains("Balance")
                                                || line.contains("Failed")
                                                || line.contains("Account query")
                                                || line.contains("Configure an Anvil")
                                        })
                                        .cloned()
                                })
                                .or_else(|| data.info.first().cloned())
                                .unwrap_or_else(|| "No account data available.".into());
                            let row_count =
                                cached_rows.as_ref().map(|rows| rows.len()).unwrap_or(0);
                            self.state.current_address = Some(data);
                            self.state.address_transactions_view.clamp(row_count);
                            if let Some(rows) = cached_rows {
                                for row in rows {
                                    self.state
                                        .transaction_preview_cache
                                        .insert(row.hash.clone(), row);
                                }
                            }
                            self.show_status(status_message);
                            self.dispatch(Action::LoadingFinished(FocusedPane::MainView));
                        }
                    }
                }
                Message::TransactionHydrated(data) => {
                    if let Some(SelectedEntity::Transaction(tx)) = self.state.selected.as_ref() {
                        if tx.hash == data.identifier {
                            self.state.current_transaction = Some(data);
                            self.dispatch(Action::LoadingFinished(FocusedPane::MainView));
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn build_address_view(
    addr: AddressRef,
    overview: Option<AccountOverview>,
    note: Option<String>,
    rpc_endpoint: Option<String>,
    block_note: Option<String>,
) -> HydratedAddress {
    let mut info = Vec::new();
    let mut transactions = Vec::new();

    if let Some(url) = rpc_endpoint.as_ref() {
        info.push(format!("RPC endpoint: {url}"));
    }

    if let Some(summary) = overview.as_ref() {
        info.push(format!("Latest block: {}", summary.latest_block));
        let balance_eth = format_units(summary.balance_wei, "ether")
            .unwrap_or_else(|_| summary.balance_wei.to_string());
        info.push(format!(
            "Balance: {} ETH ({} wei)",
            balance_eth, summary.balance_wei
        ));
        info.push(format!(
            "Transaction count (nonce): {}",
            summary.transaction_count
        ));
        info.push(format!(
            "Account type: {}",
            if summary.is_contract {
                "Contract"
            } else {
                "Externally Owned Account"
            }
        ));
    }

    if let Some(block_line) = block_note {
        info.push(block_line);
    }

    if let Some(message) = note {
        info.push(message);
    }

    if info.is_empty() {
        info.push("No account data available.".into());
    }

    if transactions.is_empty() {
        transactions.push("Transactions will appear once data is fetched.".into());
    }

    let internal = vec!["Internal transactions not yet implemented.".into()];
    let balances = vec!["Balance inspection not yet implemented.".into()];
    let permissions = vec!["Permission analysis not yet implemented.".into()];

    HydratedAddress {
        identifier: addr.address,
        info,
        transactions,
        transactions_table: None,
        internal,
        balances,
        permissions,
        overview,
    }
}

fn format_eth_value(value: &U256) -> String {
    if value.is_zero() {
        return "0 ETH".into();
    }
    match format_units(*value, "ether") {
        Ok(mut eth) => {
            trim_decimal(&mut eth);
            if eth.is_empty() {
                "0 ETH".into()
            } else {
                format!("{eth} ETH")
            }
        }
        Err(_) => format!("{value} wei"),
    }
}

fn trim_decimal(value: &mut String) {
    if let Some(_) = value.find('.') {
        while value.ends_with('0') {
            value.pop();
        }
        if value.ends_with('.') {
            value.pop();
        }
    }
}

enum TabDirection {
    Previous,
    Next,
}

enum Movement {
    Left,
    Right,
    Up,
    Down,
}

/// Immutable state shared across components.
#[derive(Debug, Default)]
pub struct AppState {
    pub navigation: NavigationState,
    pub loading: LoadingState,
    pub selected: Option<SelectedEntity>,
    pub search_error: Option<String>,
    pub secrets: SecretsState,
    pub favorite_addresses: HashSet<String>,
    pub favorite_transactions: HashSet<String>,
    pub current_address: Option<HydratedAddress>,
    pub current_transaction: Option<HydratedTransaction>,
    pub address_transactions_view: AddressTransactionsViewState,
    pub pending_transaction_preview: Option<AddressTransactionRow>,
    pub transaction_preview_cache: HashMap<String, AddressTransactionRow>,
}

#[derive(Debug, Default)]
pub struct AddressTransactionsViewState {
    pub selected_index: usize,
}

impl AddressTransactionsViewState {
    pub fn reset(&mut self) {
        self.selected_index = 0;
    }

    pub fn clamp(&mut self, len: usize) {
        if len == 0 {
            self.reset();
        } else if self.selected_index >= len {
            self.selected_index = len.saturating_sub(1);
        }
    }
}

impl AppState {
    pub fn is_favorite(&self, entity: &SelectedEntity) -> bool {
        match entity {
            SelectedEntity::Address(addr) => self.favorite_addresses.contains(&addr.address),
            SelectedEntity::Transaction(tx) => self.favorite_transactions.contains(&tx.hash),
        }
    }
}

#[derive(Debug, Default)]
pub struct NavigationState {
    pub focused_pane: FocusedPane,
    pub modal_return_focus: FocusedPane,
    pub sidebar_tab: SidebarTab,
    pub main_view_mode: MainViewMode,
    pub main_view_tab: MainViewTab,
}

impl NavigationState {
    pub fn focus_pane(&mut self, pane: FocusedPane) {
        match pane {
            FocusedPane::Modal => self.focus_modal(),
            other => {
                self.focused_pane = other;
                self.modal_return_focus = other;
            }
        }
    }

    pub fn focus_modal(&mut self) {
        if self.focused_pane != FocusedPane::Modal {
            self.modal_return_focus = self.focused_pane;
        }
        self.focused_pane = FocusedPane::Modal;
    }

    pub fn restore_focus_after_modal(&mut self) {
        self.focused_pane = self.modal_return_focus;
        self.modal_return_focus = self.focused_pane;
    }

    pub fn focus_next(&mut self) {
        let next = match self.focused_pane {
            FocusedPane::Top => FocusedPane::Sidebar,
            FocusedPane::Sidebar => FocusedPane::MainView,
            FocusedPane::MainView => FocusedPane::BottomBar,
            FocusedPane::BottomBar | FocusedPane::Modal => FocusedPane::Top,
        };
        self.focus_pane(next);
    }

    pub fn focus_previous(&mut self) {
        let previous = match self.focused_pane {
            FocusedPane::Top => FocusedPane::BottomBar,
            FocusedPane::Sidebar => FocusedPane::Top,
            FocusedPane::MainView => FocusedPane::Sidebar,
            FocusedPane::BottomBar => FocusedPane::MainView,
            FocusedPane::Modal => self.modal_return_focus,
        };
        self.focus_pane(previous);
    }

    pub fn next_main_view_tab(&mut self) {
        self.main_view_tab = self.main_view_tab.next(self.main_view_mode);
    }

    pub fn previous_main_view_tab(&mut self) {
        self.main_view_tab = self.main_view_tab.previous(self.main_view_mode);
    }
}

#[derive(Debug, Default)]
pub struct LoadingState {
    pub top: PaneLoading,
    pub sidebar: PaneLoading,
    pub main_view: PaneLoading,
}

impl LoadingState {
    pub fn set_loading(&mut self, pane: FocusedPane, value: bool) {
        let target = match pane {
            FocusedPane::Top => &mut self.top,
            FocusedPane::Sidebar => &mut self.sidebar,
            FocusedPane::MainView => &mut self.main_view,
            FocusedPane::BottomBar | FocusedPane::Modal => return,
        };
        target.is_loading = value;
        target.started_at = if value { Some(Instant::now()) } else { None };
    }
}

#[derive(Debug, Default)]
pub struct PaneLoading {
    pub is_loading: bool,
    pub started_at: Option<Instant>,
}

/// Mutable context passed to components while handling logic.
pub struct AppContext<'a> {
    pub state: &'a mut AppState,
    pub storage: &'a mut Storage,
    pub commands: CommandBus,
}

/// Read-only context used during rendering.
pub struct AppView<'a> {
    pub state: &'a AppState,
}

#[derive(Clone)]
pub struct CommandBus {
    sender: mpsc::Sender<Message>,
    handle: Handle,
}

impl CommandBus {
    pub fn new(sender: mpsc::Sender<Message>, handle: Handle) -> Self {
        Self { sender, handle }
    }

    pub fn spawn_async<F, Fut>(&self, task: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Message> + Send + 'static,
    {
        let sender = self.sender.clone();
        self.handle.spawn(async move {
            let message = task().await;
            let _ = sender.send(message);
        });
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    SearchCompleted {
        query: String,
        entity: SelectedEntity,
    },
    SearchFailed {
        query: String,
        error: String,
    },
    AddressHydrated(HydratedAddress),
    TransactionHydrated(HydratedTransaction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    FocusPane(FocusedPane),
    FocusNextPane,
    FocusPreviousPane,
    SelectionChanged(SelectedEntity),
    LoadingStarted(FocusedPane),
    LoadingFinished(FocusedPane),
    CloseModal,
    SecretsSaved,
}

mod navigation {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FocusedPane {
        Top,
        Sidebar,
        MainView,
        BottomBar,
        #[allow(dead_code)]
        Modal,
    }

    impl FocusedPane {
        pub fn from_number(number: usize) -> Option<Self> {
            match number {
                1 => Some(Self::Top),
                2 => Some(Self::Sidebar),
                3 => Some(Self::MainView),
                4 => Some(Self::BottomBar),
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SidebarTab {
        Addresses,
        Transactions,
    }

    impl SidebarTab {
        pub fn next(self) -> Self {
            match self {
                SidebarTab::Addresses => SidebarTab::Transactions,
                SidebarTab::Transactions => SidebarTab::Addresses,
            }
        }

        pub fn previous(self) -> Self {
            match self {
                SidebarTab::Addresses => SidebarTab::Transactions,
                SidebarTab::Transactions => SidebarTab::Addresses,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MainViewMode {
        Address,
        Transaction,
    }

    impl Default for MainViewMode {
        fn default() -> Self {
            Self::Address
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MainViewTab {
        AddressInfo,
        AddressTransactions,
        AddressInternal,
        AddressBalances,
        AddressPermissions,
        TransactionSummary,
        TransactionDebug,
        TransactionStorageDiff,
    }

    impl Default for MainViewTab {
        fn default() -> Self {
            Self::AddressInfo
        }
    }

    impl MainViewTab {
        pub fn normalize(self, mode: MainViewMode) -> Self {
            match mode {
                MainViewMode::Address => match self {
                    MainViewTab::AddressInfo
                    | MainViewTab::AddressTransactions
                    | MainViewTab::AddressInternal
                    | MainViewTab::AddressBalances
                    | MainViewTab::AddressPermissions => self,
                    _ => MainViewTab::AddressInfo,
                },
                MainViewMode::Transaction => match self {
                    MainViewTab::TransactionSummary
                    | MainViewTab::TransactionDebug
                    | MainViewTab::TransactionStorageDiff => self,
                    _ => MainViewTab::TransactionSummary,
                },
            }
        }

        pub fn next(self, mode: MainViewMode) -> Self {
            match mode {
                MainViewMode::Address => match self.normalize(mode) {
                    MainViewTab::AddressInfo => MainViewTab::AddressTransactions,
                    MainViewTab::AddressTransactions => MainViewTab::AddressInternal,
                    MainViewTab::AddressInternal => MainViewTab::AddressBalances,
                    MainViewTab::AddressBalances => MainViewTab::AddressPermissions,
                    MainViewTab::AddressPermissions => MainViewTab::AddressInfo,
                    other => other,
                },
                MainViewMode::Transaction => match self.normalize(mode) {
                    MainViewTab::TransactionSummary => MainViewTab::TransactionDebug,
                    MainViewTab::TransactionDebug => MainViewTab::TransactionStorageDiff,
                    MainViewTab::TransactionStorageDiff => MainViewTab::TransactionSummary,
                    other => other,
                },
            }
        }

        pub fn previous(self, mode: MainViewMode) -> Self {
            match mode {
                MainViewMode::Address => match self.normalize(mode) {
                    MainViewTab::AddressInfo => MainViewTab::AddressPermissions,
                    MainViewTab::AddressTransactions => MainViewTab::AddressInfo,
                    MainViewTab::AddressInternal => MainViewTab::AddressTransactions,
                    MainViewTab::AddressBalances => MainViewTab::AddressInternal,
                    MainViewTab::AddressPermissions => MainViewTab::AddressBalances,
                    other => other,
                },
                MainViewMode::Transaction => match self.normalize(mode) {
                    MainViewTab::TransactionSummary => MainViewTab::TransactionStorageDiff,
                    MainViewTab::TransactionDebug => MainViewTab::TransactionSummary,
                    MainViewTab::TransactionStorageDiff => MainViewTab::TransactionDebug,
                    other => other,
                },
            }
        }
    }

    impl Default for FocusedPane {
        fn default() -> Self {
            Self::Top
        }
    }

    impl Default for SidebarTab {
        fn default() -> Self {
            Self::Addresses
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tempfile::tempdir;

    #[test]
    fn secrets_modal_accepts_urls() -> AppResult<()> {
        let tmp = tempdir().unwrap();
        unsafe {
            std::env::set_var("EVM_TUI_DATA_DIR", tmp.path());
        }

        let mut app = App::new()?;
        assert!(app.secrets_modal_mut().is_some());

        app.handle_modal_paste("H43UPPAU7H4KBX99TSWMD3IHDG9F86IK43".into())?;
        app.handle_modal_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;
        let url = "https://eth-mainnet.g.alchemy.com/v2/example-key";
        app.handle_modal_paste(url.into())?;
        app.handle_modal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;

        assert_eq!(app.state.secrets.anvil_rpc_url.as_deref(), Some(url));

        unsafe {
            std::env::remove_var("EVM_TUI_DATA_DIR");
        }
        Ok(())
    }
}
