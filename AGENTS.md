# Repository Guidelines

## Project Structure & Module Organization
`src/main.rs` bootstraps Crossterm, Ratatui, and error reporting. Keep it thin: route UI rendering to modules under `src/ui/` (`top.rs`, `sidebar/`, `main_view/`, `bottom_bar.rs`), keep shared app state and actions in `src/app/`, and share the `Component` trait from `src/components/`. Fjall persistence lives in `src/storage/` with per-entity partitions (e.g., `storage/favorites_addresses`, `storage/favorites_txs`). Specs live in `specs/`; update when panes or flows change.

## Architecture & UI Panels
The UI exposes three numbered panes—Top (header, search, settings), Application (sidebar + main view), and Bottom Bar (key help). Sidebar tabs must scale to hundreds of chains; seed with Mainnet, Arbitrum, Base, Sepolia and label entries with chain names. Main view swaps between address tabs (transactions, internal txs, balances, permissions) and transaction tabs (summary, debugging, storage diff). Hydrate all tabs on selection and apply the shared loading language (`specs/loading_refresh.md`) for spinners, shimmer accents, and bottom-bar status.

## Build & Development Commands
- `cargo fmt` – enforce formatting; use `-- --check` before pushing.
- `cargo clippy --all-targets --all-features` – warnings break the build.
- `cargo build` / `cargo run` – compile or launch the TUI (`--release` for profiling).
- `cargo test` – run before every commit.

## Coding Style, Input & UX
Stick to four-space indent and idiomatic Rust naming (`snake_case`, `UpperCamelCase`, `SCREAMING_SNAKE_CASE`). Structure widgets so each pane exposes a `Component`-like trait handling input, draw, and tick. Default navigation: `[` and `]` cycle tabs; `h/j/k/l` move within panes; numeric hotkeys (e.g., `3`) focus panes; `q` exits. Top-bar filters toggle with `f`, sidebar grouping toggles with `g`, and storage diff exports trigger via `e`. Favor community Ratatui widgets (progress indicators, tab bars) when they improve clarity.

## Environment & Integrations
Access Alloy for chain data and Fjall for persistence; inject dependencies for mocking. Read `ETHERSCAN_API_KEY` (env or settings) to fetch contract source/ABI for debugger views; degrade gracefully when absent. If providers throttle requests, raise a dialog with the error message rather than retry loops.

## Testing Guidelines
Keep unit tests near logic (`#[cfg(test)]` modules). Add integrations under `tests/` for hydration pipelines, tab switching, and error dialogs; mock Alloy/Fjall via traits or in-memory adapters. Use snapshot tests to guard key-binding focus. Run `cargo test` before every PR.

## Commit & Pull Request Guidelines
Favor Conventional Commit headers (`feat: sidebar favorites storage`). PRs must note spec updates, include terminal recordings for UI changes, list executed commands, and reference issues (`Closes #123`). Keep diffs reviewable in <15 minutes; split structural refactors from features.
