# Bottom Bar Spec

## Purpose
Provide contextual keyboard hints and status indicators without occupying valuable vertical space.

## Content
- Always show global shortcuts (`q Quit`, `[ / ] Tabs`, `h/j/k/l Move`, `1..9 Focus`).
- Secondary region displays context-sensitive actions from the currently focused pane (e.g., `Enter Open`, `d Remove Favorite`).
- Reserve a right-aligned slot for transient status (sync progress, rate-limit warnings).
- Shortcut order is fixed to match documentation; no user reordering in MVP.
- Prepend statuses with the shared spinner and shimmer pill described in `loading_refresh.md`.

## Behavior
- React to focus changes via shared app state so hints update instantly.
- When a modal is open, collapse to modal-specific shortcuts and dim standard hints.
- Animate subtle color change when new hints appear to draw attention.
- Constrain layout to a single line; handle overflow via horizontal scrolling rather than wrapping.

## Implementation Notes
- Build with Ratatui `Paragraph` or `Line` composing styled spans.
- Consider `ratatui-textarea` or similar for dynamic hint alignment.
- Keep height to a single line; truncate overflow with ellipsis or scrolling.
