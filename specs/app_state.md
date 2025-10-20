# Application State & Component Pattern

## Core Concepts
- `App` orchestrates the event loop, stores global context (focus, theme, loading flags), and owns component instances.
- `AppState` holds immutable configuration (theme preferences, keymap) and shared mutable state (active pane, selections, hydration flags).
- `Action` represents user intent or async responses (`FocusPane`, `SelectTab`, `HydrationStarted`, `HydrationFinished`, etc.).
- `Message` bridges background tasks back to the UI thread via an async channel; messages translate into actions.

## Component Trait
```rust
pub trait Component {
    type Action;
    fn init(&mut self, ctx: &mut AppContext) -> anyhow::Result<()>;
    fn handle(&mut self, action: &Self::Action, ctx: &mut AppContext) -> anyhow::Result<Option<Action>>;
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &AppContext);
    fn tick(&mut self, now: Instant, ctx: &mut AppContext) -> anyhow::Result<Option<Action>>;
}
```
- `Action` is component-local; returned actions are lifted to global `Action` by the caller.
- `AppContext` provides access to shared state, repositories, and theme.

## Focus & Selection
- `FocusedPane` enum tracks which pane owns keyboard input (`Top`, `Sidebar`, `MainView`, `BottomBar`, `Modal`).
- Each pane tracks its own tab (`TopTab`, `SidebarTab`, `MainViewTab`).
- `NavigationState` consolidates focus, tab indices, and selection pointers; expose helpers for cycling with `[`, `]`, and `h/j/k/l`.

## Loading Flags
- Shared `LoadingState` from `specs/loading_refresh.md`: per-pane booleans plus timestamps (`Option<Instant>`).
- Components mark refresh start/end by dispatching `Action::LoadingStarted(Pane)` and `Action::LoadingFinished(Pane)`.

## Fjall Integration
- Initialize Fjall once in `App::new` via `Storage::new(data_dir)` returning a handle with named partitions (`favorites_addresses`, `favorites_transactions`, `settings`).
- Repositories expose async-like APIs but return `Result` immediately; spawn real async jobs later using `tokio::task::spawn` when networking arrives.
- Store handles in `AppContext` so components query/update favorites without direct Fjall usage.

## Event Flow
1. Crossterm event -> `App::on_event` -> mapped to `Action`.
2. `App::dispatch` routes the action to the focused component, returns optional follow-up `Action`.
3. Side effects (e.g., start hydration task) push `Message` onto async channel.
4. Main loop polls message channel via `try_recv` and converts to actions on each tick.

## Testing Strategy
- Components implement dependency-free logic and can be unit-tested by driving `handle` and `render` with mocked `AppContext`.
- Integration tests spin a headless terminal (`ratatui::backend::TestBackend`) to snapshot layout and ensure focus transitions obey keymap.
