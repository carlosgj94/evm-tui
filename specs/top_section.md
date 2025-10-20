# Top Section Spec

## Purpose
Surface global context and quick actions regardless of the focused pane.

## Layout
- **Header Title**: Left-aligned, reflects current selection (address hash, transaction hash) or app default.
- **Search Bar**: Center column; supports address and transaction queries with validation feedback inline.
- **Chain Filter**: Compact dropdown/button adjacent to the search, toggled with `f`, allowing quick restriction to specific networks.
- **Settings Button**: Right-aligned icon/button; opens modal with configuration (API keys, theme, chain filters) and displays badge counters for pending tasks.

## Behaviors
- Update title on every selection change; show loading suffix (e.g., `…`) while data hydrates.
- Search input should debounce network lookups and offer history suggestions.
- Chain filter reflects active network scope and supports multi-select via spacebar.
- Settings button triggers modal while preserving pane focus state for return and displays a warning badge when required configuration (e.g., `ETHERSCAN_API_KEY`) is missing.

## Visuals & Widgets
- Use Ratatui `Layout` with fixed min height.
- Prefer third-party widget for search (e.g., `ratatui-extras` input) if it supports cursor + history out of the box; otherwise wrap a custom widget.
- Follow `loading_refresh.md` for spinners and shimmer underline; render the shared spinner inside the search box and shimmer the header while global refresh runs.
