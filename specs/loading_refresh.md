# Loading & Refresh Spec

## Goals
- Present a cohesive loading language across panes without hiding existing data.
- Support light and dark terminals automatically while still looking distinctive.
- Reuse shared components so animations and colors stay in sync.

## Visual Language
- **Base Spinner**: Use `throbber-widgets-tui` for all animated spinners. Standard form is the `CLOCK` preset with label text. Slow tick (150 ms) for background refresh, faster (80 ms) for blocking states.
- **Shimmer Accent**: Adapt Codex-style shimmer (diagonal gradient sweep) for emphasis on active refresh zones. Use sparingly: header title updates, active tab underline, and bottom-bar status pill. Fall back to a static gradient when the terminal lacks true color.
- **Palette Adaptation**: Derive colors from the current terminal palette. Start from Ratatui’s `Palette16` when available; otherwise, detect light vs dark by sampling the background color. Invert shimmer highlight for light backgrounds and keep contrast ≥ 4.5:1.

## Component Placement
- **Top Pane**: Display a spinner inside the search field during remote lookups. Apply a shimmer underline to the header while global hydration is in flight. Settings badge flashes shimmer when critical configuration is missing.
- **Sidebar**: Show mini spinners beside chain group headers during refresh. Keep list entries visible; fade text to 70 % opacity while data is stale.
- **Main View**: Overlay a translucent shimmer bar across the active tab label during fetch, and place a centered spinner in the content area only when no prior data exists. For incremental refresh, pin a spinner to the tab bar corner instead.
- **Bottom Bar**: Always render the global spinner plus status text. When a pane is refreshing, prepend its pane number (`[2] Refreshing Favorites…`) and shimmer the status pill.

## State Modeling
- Centralize loading state in `AppLoadingState` with per-pane flags (`top`, `sidebar`, `main_view`) and timestamps. Provide helpers (`begin_refresh`, `end_refresh`, `is_stale`) so widgets can decide between blocking vs background visuals.
- Emit events when refresh spans exceed thresholds (e.g., 5 s) to escalate bottom-bar messaging from spinner to warning banner.

## Implementation Notes
- Introduce a shared `theme::LoadingTheme` that exposes colors for spinner, shimmer gradient, and muted text; recompute when the terminal signals a theme change (`Terminal::autoresize`).
- Guard shimmer drawing with feature detection; if true color is unavailable, switch to a two-tone gradient and reduce animation rate to avoid flicker.
- Expose a utility to render shimmer as a single function (`render_shimmer(area, intensity)`) so panes can hook the behavior consistently.
