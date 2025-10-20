# Main View Spec

## Purpose
Display detailed information for the active address or transaction and host advanced tooling.

## Address Layout
- Tabs: `Transactions`, `Internal`, `Balances`, `Permissions`.
- Default to Transactions list with pagination and filters by chain or method signature.
- Internal tab surfaces internal calls with call tree visualization.
- Balances tab aggregates token balances (native and ERC20) with fiat estimates when available.
- Permissions tab lists contracts where the address has roles; highlight high-risk scopes.

## Transaction Layout
- Tabs: `Summary`, `Debug`, `Storage Diff`.
- Summary renders block, value, gas, participants, decoded call data, and status badges.
- Debug tab integrates Alloy tracing to step through opcodes and, where ABI is available, source-level playback similar to Tenderly.
- Storage Diff tab compares pre/post state for touched contracts; highlight write hotspots and expose an `e` keybinding to export the diff as JSON under `exports/<tx_hash>.json`.

## Hydration Flow
- On selection, launch parallel fetches for every tab; render placeholders immediately and follow `loading_refresh.md`—centered spinner when empty, tab-bar shimmer for incremental refresh.
- Cache recent responses in memory keyed by `(entity, chain)` and refresh in the background with stale-while-revalidate semantics.
- Missing data or throttling should render callouts instead of collapsing the tab.
- Simulation/what-if tooling is deferred beyond MVP; capture requirements in specs when prioritised.

## External Dependencies
- Alloy transports for EVM RPC and tracing (configurable per chain).
- Etherscan family APIs for contract source/ABI; require `ETHERSCAN_API_KEY`.
- Optional integration points for alternative providers (Tenderly, Blockscout) tracked as follow-up enhancements, with simulation tooling documented separately.
