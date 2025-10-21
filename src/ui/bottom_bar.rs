use crate::{
    app::{Action, AppContext, AppResult, AppView, FocusedPane},
    components::Component,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Paragraph},
};

#[derive(Debug, Default)]
pub struct BottomBar;

#[allow(dead_code)]
#[derive(Debug)]
pub enum BottomBarCommand {
    UpdateStatus(String),
}

impl Component for BottomBar {
    type Command = BottomBarCommand;

    fn init(&mut self, _ctx: &mut AppContext<'_>) -> AppResult<()> {
        Ok(())
    }

    fn update(
        &mut self,
        _command: &Self::Command,
        _ctx: &mut AppContext<'_>,
    ) -> AppResult<Option<Action>> {
        Ok(None)
    }

    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &AppView<'_>) {
        let is_focused = matches!(ctx.state.navigation.focused_pane, FocusedPane::BottomBar);
        let style = if is_focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        let widget = Paragraph::new(Line::from(
            "q Quit • [ Prev Tab • ] Next Tab • h j k l Move • Enter Open • 1..9 Focus • [F] Favorite/Remove",
        ))
        .block(Block::bordered().title(Line::from("[4] Keymap").style(style)));
        frame.render_widget(widget, area);
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>) -> AppResult<Option<Action>> {
        Ok(None)
    }
}
