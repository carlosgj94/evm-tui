use super::util::short_hex;
use crate::{
    app::{
        Action, AddressRef, AppContext, AppResult, AppView, FocusedPane, SelectedEntity,
        SidebarTab, TransactionRef,
    },
    components::Component,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
};

#[derive(Debug)]
pub struct Sidebar {
    addresses: Vec<AddressRef>,
    transactions: Vec<TransactionRef>,
    selected_index: usize,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            addresses: Vec::new(),
            transactions: Vec::new(),
            selected_index: 0,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum SidebarCommand {
    MoveUp,
    MoveDown,
    NextTab,
    PreviousTab,
    SelectIndex(usize),
    SwitchTab(SidebarTab),
    HydrationStarted,
    HydrationFinished,
    AddFavorite(SelectedEntity),
    RemoveFavorite(SelectedEntity),
}

impl Sidebar {
    fn len(&self, tab: SidebarTab) -> usize {
        match tab {
            SidebarTab::Addresses => self.addresses.len(),
            SidebarTab::Transactions => self.transactions.len(),
        }
    }

    fn clamp_selection(&mut self, tab: SidebarTab) {
        let len = self.len(tab);
        if len == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= len {
            self.selected_index = len.saturating_sub(1);
        }
    }

    pub fn set_addresses(&mut self, items: Vec<AddressRef>, current_tab: SidebarTab) {
        self.addresses = items;
        if matches!(current_tab, SidebarTab::Addresses) {
            self.clamp_selection(SidebarTab::Addresses);
        }
    }

    pub fn set_transactions(&mut self, items: Vec<TransactionRef>, current_tab: SidebarTab) {
        self.transactions = items;
        if matches!(current_tab, SidebarTab::Transactions) {
            self.clamp_selection(SidebarTab::Transactions);
        }
    }

    fn selected_entity(&self, tab: SidebarTab, index: usize) -> Option<SelectedEntity> {
        match tab {
            SidebarTab::Addresses => self
                .addresses
                .get(index)
                .map(|addr| SelectedEntity::Address(addr.clone())),
            SidebarTab::Transactions => self
                .transactions
                .get(index)
                .map(|tx| SelectedEntity::Transaction(tx.clone())),
        }
    }

    pub fn current_selection(&self, tab: SidebarTab, index: usize) -> Option<SelectedEntity> {
        self.selected_entity(tab, index)
    }

    pub fn active_selection(&self, tab: SidebarTab) -> Option<SelectedEntity> {
        self.selected_entity(tab, self.selected_index)
    }

    fn display_label(&self, tab: SidebarTab, index: usize) -> String {
        match tab {
            SidebarTab::Addresses => self
                .addresses
                .get(index)
                .map(|addr| format!("{} [{}]", short_hex(&addr.address), addr.chain))
                .unwrap_or_default(),
            SidebarTab::Transactions => self
                .transactions
                .get(index)
                .map(|tx| format!("{} • {}", tx.chain, tx.label))
                .unwrap_or_default(),
        }
    }
}

impl Component for Sidebar {
    type Command = SidebarCommand;

    fn init(&mut self, _ctx: &mut AppContext<'_>) -> AppResult<()> {
        Ok(())
    }

    fn update(
        &mut self,
        command: &Self::Command,
        ctx: &mut AppContext<'_>,
    ) -> AppResult<Option<Action>> {
        let mut selection_changed = false;
        match command {
            SidebarCommand::MoveUp => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    selection_changed = true;
                }
            }
            SidebarCommand::MoveDown => {
                let len = self.len(ctx.state.navigation.sidebar_tab);
                if len > 0 {
                    self.selected_index = (self.selected_index + 1).min(len.saturating_sub(1));
                    selection_changed = true;
                }
            }
            SidebarCommand::NextTab => {
                ctx.state.navigation.sidebar_tab = ctx.state.navigation.sidebar_tab.next();
                self.selected_index = 0;
                self.clamp_selection(ctx.state.navigation.sidebar_tab);
                selection_changed = true;
            }
            SidebarCommand::PreviousTab => {
                ctx.state.navigation.sidebar_tab = ctx.state.navigation.sidebar_tab.previous();
                self.selected_index = 0;
                self.clamp_selection(ctx.state.navigation.sidebar_tab);
                selection_changed = true;
            }
            SidebarCommand::SelectIndex(index) => {
                let len = self.len(ctx.state.navigation.sidebar_tab);
                if len > 0 {
                    self.selected_index = (*index).min(len - 1);
                    selection_changed = true;
                }
            }
            SidebarCommand::SwitchTab(tab) => {
                ctx.state.navigation.sidebar_tab = *tab;
                self.selected_index = 0;
                self.clamp_selection(*tab);
                selection_changed = true;
            }
            SidebarCommand::HydrationStarted | SidebarCommand::HydrationFinished => {}
            SidebarCommand::AddFavorite(entity) => {
                let current_tab = ctx.state.navigation.sidebar_tab;
                match entity {
                    SelectedEntity::Address(addr) => {
                        if !self.addresses.iter().any(|a| a.address == addr.address) {
                            self.addresses.insert(0, addr.clone());
                            if current_tab == SidebarTab::Addresses {
                                self.selected_index = 0;
                                selection_changed = true;
                            }
                        }
                    }
                    SelectedEntity::Transaction(tx) => {
                        if !self.transactions.iter().any(|t| t.hash == tx.hash) {
                            self.transactions.insert(0, tx.clone());
                            if current_tab == SidebarTab::Transactions {
                                self.selected_index = 0;
                                selection_changed = true;
                            }
                        }
                    }
                }
                if current_tab == SidebarTab::Addresses {
                    self.clamp_selection(SidebarTab::Addresses);
                } else {
                    self.clamp_selection(SidebarTab::Transactions);
                }
            }
            SidebarCommand::RemoveFavorite(entity) => {
                let current_tab = ctx.state.navigation.sidebar_tab;
                match entity {
                    SelectedEntity::Address(addr) => {
                        if self.addresses.iter().any(|a| a.address == addr.address) {
                            self.addresses.retain(|a| a.address != addr.address);
                            if current_tab == SidebarTab::Addresses {
                                selection_changed = true;
                            }
                        }
                    }
                    SelectedEntity::Transaction(tx) => {
                        if self.transactions.iter().any(|t| t.hash == tx.hash) {
                            self.transactions.retain(|t| t.hash != tx.hash);
                            if current_tab == SidebarTab::Transactions {
                                selection_changed = true;
                            }
                        }
                    }
                }
                if current_tab == SidebarTab::Addresses {
                    self.clamp_selection(SidebarTab::Addresses);
                } else {
                    self.clamp_selection(SidebarTab::Transactions);
                }
            }
        }
        if selection_changed {
            if let Some(entity) =
                self.selected_entity(ctx.state.navigation.sidebar_tab, self.selected_index)
            {
                return Ok(Some(Action::SelectionChanged(entity)));
            }
        }
        Ok(None)
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &AppView<'_>) {
        let is_focused = matches!(ctx.state.navigation.focused_pane, FocusedPane::Sidebar);
        let border_style = if is_focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from("[2] Favorites").style(border_style));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(inner);

        let tab_titles = vec![Line::from("Addresses"), Line::from("Transactions")];
        let tab_index = match ctx.state.navigation.sidebar_tab {
            SidebarTab::Addresses => 0,
            SidebarTab::Transactions => 1,
        };
        let tabs = Tabs::new(tab_titles)
            .select(tab_index)
            .style(Style::default())
            .highlight_style(Style::default().fg(Color::Cyan));
        frame.render_widget(tabs, chunks[0]);

        let len = self.len(ctx.state.navigation.sidebar_tab);
        if len == 0 {
            let empty = Paragraph::new("No favorites yet. Press `a` to add one.")
                .style(Style::default().fg(Color::Gray));
            frame.render_widget(empty, chunks[1]);
            return;
        }

        let list_items: Vec<ListItem> = (0..len)
            .map(|i| {
                let label = self.display_label(ctx.state.navigation.sidebar_tab, i);
                ListItem::new(label)
            })
            .collect();
        let highlight = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let mut state = ListState::default();
        state.select(Some(self.selected_index));

        let list = List::new(list_items)
            .highlight_style(highlight)
            .highlight_symbol("▸ ");
        frame.render_stateful_widget(list, chunks[1], &mut state);
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>) -> AppResult<Option<Action>> {
        Ok(None)
    }
}
