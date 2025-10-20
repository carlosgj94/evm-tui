use crate::app::{Action, AppContext, AppResult, AppView};
use ratatui::Frame;
use ratatui::layout::Rect;

/// Trait implemented by all UI components (panes, modals, etc.).
pub trait Component {
    /// Component-local action type. Returned actions will be lifted into the global [`Action`].
    type Command;

    /// Perform setup logic such as loading persisted state.
    fn init(&mut self, ctx: &mut AppContext<'_>) -> AppResult<()>;

    /// Handle a component-local command and optionally bubble up a global action.
    fn update(
        &mut self,
        command: &Self::Command,
        ctx: &mut AppContext<'_>,
    ) -> AppResult<Option<Action>>;

    /// Render the component into the provided [`Rect`].
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &AppView<'_>);

    /// Called on every tick to perform periodic work (e.g., animation, polling).
    fn tick(&mut self, ctx: &mut AppContext<'_>) -> AppResult<Option<Action>>;
}
