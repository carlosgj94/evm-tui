use crate::{
    components::Component,
    storage::{FavoriteRecord, Storage},
    ui::util::short_hex,
    ui::{
        bottom_bar::BottomBar,
        main_view::{MainView, MainViewCommand},
        sidebar::{Sidebar, SidebarCommand},
        top::{TopBar, TopCommand},
    },
};
pub type AppResult<T> = color_eyre::Result<T>;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
};
use std::{collections::HashSet, sync::mpsc, thread, time::Instant};

use tokio::runtime::{Handle, Runtime};
use tokio::time::{sleep, Duration};

pub use navigation::{FocusedPane, MainViewMode, MainViewTab, SidebarTab};

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
    pub transactions: Vec<String>,
    pub internal: Vec<String>,
    pub balances: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydratedTransaction {
    pub identifier: String,
    pub summary: Vec<String>,
    pub debug: Vec<String>,
    pub storage_diff: Vec<String>,
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
    runtime_handle: Handle,
    message_rx: mpsc::Receiver<Message>,
    message_tx: mpsc::Sender<Message>,
}

impl App {
    pub fn new() -> AppResult<Self> {
        let mut state = AppState::default();
        let mut storage = Storage::open_default()?;
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

        Ok(Self {
            running: false,
            state,
            storage,
            top_bar,
            sidebar,
            main_view,
            bottom_bar,
            runtime,
            runtime_handle,
            message_rx,
            message_tx,
        })
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
                Constraint::Length(2),
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
    }

    fn handle_events(&mut self) -> AppResult<()> {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key_event(key)?,
            Event::Mouse(_) | Event::Resize(_, _) => {}
            _ => {}
        }
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) -> AppResult<()> {
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

    fn dispatch(&mut self, action: Action) {
        match action {
            Action::Quit => self.running = false,
            Action::FocusPane(pane) => self.state.navigation.focused_pane = pane,
            Action::FocusNextPane => self.state.navigation.focus_next(),
            Action::FocusPreviousPane => self.state.navigation.focus_previous(),
            Action::SelectionChanged(entity) => {
                self.state.selected = Some(entity.clone());
                self.state.search_error = None;
                match entity {
                    SelectedEntity::Address(_) => {
                        self.state.navigation.main_view_mode = MainViewMode::Address;
                        self.state.navigation.main_view_tab = MainViewTab::AddressTransactions;
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
            Action::Noop => {}
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
                Movement::Left | Movement::Right | Movement::Up | Movement::Down => {}
            },
            FocusedPane::Top | FocusedPane::BottomBar | FocusedPane::Modal => {}
        }
        Ok(())
    }

    fn sidebar_command(&mut self, command: SidebarCommand) -> AppResult<()> {
        let mut ctx = AppContext {
            state: &mut self.state,
            storage: &mut self.storage,
            commands: CommandBus::new(self.message_tx.clone(), self.runtime_handle.clone()),
        };
        if let Some(action) = self.sidebar.update(&command, &mut ctx)? {
            self.dispatch(action);
        }
        Ok(())
    }

    fn main_view_command(&mut self, command: MainViewCommand) -> AppResult<()> {
        let mut ctx = AppContext {
            state: &mut self.state,
            storage: &mut self.storage,
            commands: CommandBus::new(self.message_tx.clone(), self.runtime_handle.clone()),
        };
        if let Some(action) = self.main_view.update(&command, &mut ctx)? {
            self.dispatch(action);
        }
        Ok(())
    }

    fn top_bar_command(&mut self, command: TopCommand) -> AppResult<()> {
        let mut ctx = AppContext {
            state: &mut self.state,
            storage: &mut self.storage,
            commands: CommandBus::new(self.message_tx.clone(), self.runtime_handle.clone()),
        };
        if let Some(action) = self.top_bar.update(&command, &mut ctx)? {
            self.dispatch(action);
        }
        Ok(())
    }

    fn command_bus(&self) -> CommandBus {
        CommandBus::new(self.message_tx.clone(), self.runtime_handle.clone())
    }

    fn start_hydration(&mut self, entity: SelectedEntity) {
        match entity {
            SelectedEntity::Address(addr) => {
                self.state.current_address = None;
                self.state.loading.set_loading(FocusedPane::MainView, true);
                let bus = self.command_bus();
                bus.spawn_async(move || {
                    let addr_ref = addr.clone();
                    async move {
                        sleep(Duration::from_millis(350)).await;
                        let short = short_hex(&addr_ref.address);
                        Message::AddressHydrated(HydratedAddress {
                            identifier: addr_ref.address.clone(),
                            transactions: vec![
                                format!("{} • received 1.2 ETH", short),
                                format!("{} • sent 0.5 ETH", short),
                            ],
                            internal: vec!["delegatecall → vault".into()],
                            balances: vec!["ETH: 1.23".into(), "USDC: 2,500".into()],
                            permissions: vec!["Owner: Multisig 0xABCD…".into()],
                        })
                    }
                });
            }
            SelectedEntity::Transaction(tx) => {
                self.state.current_transaction = None;
                self.state.loading.set_loading(FocusedPane::MainView, true);
                let bus = self.command_bus();
                bus.spawn_async(move || {
                    let tx_ref = tx.clone();
                    async move {
                        sleep(Duration::from_millis(350)).await;
                        let short = short_hex(&tx_ref.hash);
                        Message::TransactionHydrated(HydratedTransaction {
                            identifier: tx_ref.hash.clone(),
                            summary: vec![
                                format!("Hash: {}", short),
                                "Block: 18,551,234".into(),
                                "Gas Used: 120,000".into(),
                            ],
                            debug: vec!["step 42: CALL".into(), "step 87: SSTORE".into()],
                            storage_diff: vec!["contract 0xdead… writes slot 0x00".into()],
                        })
                    }
                });
            }
        }
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
            let (state, storage) = (&mut self.state, &mut self.storage);
            let mut ctx = AppContext {
                state,
                storage,
                commands: CommandBus::new(self.message_tx.clone(), self.runtime_handle.clone()),
            };
            if let Some(action) = self.top_bar.tick(&mut ctx)? {
                self.dispatch(action);
            }
        }
        {
            let (state, storage) = (&mut self.state, &mut self.storage);
            let mut ctx = AppContext {
                state,
                storage,
                commands: CommandBus::new(self.message_tx.clone(), self.runtime_handle.clone()),
            };
            if let Some(action) = self.sidebar.tick(&mut ctx)? {
                self.dispatch(action);
            }
        }
        {
            let (state, storage) = (&mut self.state, &mut self.storage);
            let mut ctx = AppContext {
                state,
                storage,
                commands: CommandBus::new(self.message_tx.clone(), self.runtime_handle.clone()),
            };
            if let Some(action) = self.main_view.tick(&mut ctx)? {
                self.dispatch(action);
            }
        }
        {
            let (state, storage) = (&mut self.state, &mut self.storage);
            let mut ctx = AppContext {
                state,
                storage,
                commands: CommandBus::new(self.message_tx.clone(), self.runtime_handle.clone()),
            };
            if let Some(action) = self.bottom_bar.tick(&mut ctx)? {
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
                            self.state.current_address = Some(data);
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
    pub favorite_addresses: HashSet<String>,
    pub favorite_transactions: HashSet<String>,
    pub current_address: Option<HydratedAddress>,
    pub current_transaction: Option<HydratedTransaction>,
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
    pub sidebar_tab: SidebarTab,
    pub main_view_mode: MainViewMode,
    pub main_view_tab: MainViewTab,
}

impl NavigationState {
    pub fn focus_next(&mut self) {
        self.focused_pane = match self.focused_pane {
            FocusedPane::Top => FocusedPane::Sidebar,
            FocusedPane::Sidebar => FocusedPane::MainView,
            FocusedPane::MainView => FocusedPane::BottomBar,
            FocusedPane::BottomBar | FocusedPane::Modal => FocusedPane::Top,
        };
    }

    pub fn focus_previous(&mut self) {
        self.focused_pane = match self.focused_pane {
            FocusedPane::Top => FocusedPane::BottomBar,
            FocusedPane::Sidebar => FocusedPane::Top,
            FocusedPane::MainView => FocusedPane::Sidebar,
            FocusedPane::BottomBar => FocusedPane::MainView,
            FocusedPane::Modal => FocusedPane::Top,
        };
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

    pub fn spawn<F>(&self, task: F)
    where
        F: FnOnce() -> Message + Send + 'static,
    {
        let sender = self.sender.clone();
        thread::spawn(move || {
            let message = task();
            let _ = sender.send(message);
        });
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

    pub fn send(&self, message: Message) {
        let _ = self.sender.send(message);
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
    Noop,
}

mod navigation {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FocusedPane {
        Top,
        Sidebar,
        MainView,
        BottomBar,
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
            Self::AddressTransactions
        }
    }

    impl MainViewTab {
        pub fn normalize(self, mode: MainViewMode) -> Self {
            match mode {
                MainViewMode::Address => match self {
                    MainViewTab::AddressTransactions
                    | MainViewTab::AddressInternal
                    | MainViewTab::AddressBalances
                    | MainViewTab::AddressPermissions => self,
                    _ => MainViewTab::AddressTransactions,
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
                    MainViewTab::AddressTransactions => MainViewTab::AddressInternal,
                    MainViewTab::AddressInternal => MainViewTab::AddressBalances,
                    MainViewTab::AddressBalances => MainViewTab::AddressPermissions,
                    MainViewTab::AddressPermissions => MainViewTab::AddressTransactions,
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
                    MainViewTab::AddressTransactions => MainViewTab::AddressPermissions,
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
