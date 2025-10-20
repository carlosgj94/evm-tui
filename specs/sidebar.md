# Sidebar Spec

## Purpose
Provide quick navigation for favorited addresses and transactions.

## Structure
- Pane header displays index number and icon (e.g., `1 Sidebar`).
- Tabs: `Addresses` (default) and `Transactions`.
- Lists auto-group by chain with collapsible headers when a tab exceeds 50 entries; toggle grouping with `g`.
- Each list item shows label or shortened hash plus chain name (e.g., `Base • 0x1234…abcd`).

## Data & Storage
- Favorites persist in Fjall using separate tables: `favorites_addresses` and `favorites_transactions`.
- Items store: label, canonical hash, chain id, last_viewed block height, and cached metadata timestamp.
- Hydrate entries at startup; refresh on interval or manual trigger.
- Toggle operations write-through immediately to Fjall so address/transaction stars survive restarts.

## Interactions
- `j`/`k` move selection; `Enter` activates the item and updates main view.
- `[`/`]` swap tabs; maintain per-tab cursor position.
- `d` removes the highlighted favorite (confirm dialog).
- `a` opens an add-favorite flow prefilled with the current selection.
- `g` toggles chain grouping when lists are short and a flat view is preferred.

## Loading & Feedback
- On hydration or refresh, use the shared mini spinner beside each chain header and fade stale rows per `loading_refresh.md`.
- Errors (rate limits, network) render as banner within the pane without losing selection state.
- Tagging beyond chain name is deferred; rely on grouping and sorting for MVP.
