# Data & Integrations Spec

## Persistence
- Fjall stores user data under `storage/`; create dedicated tables for addresses, transactions, settings, secrets (Etherscan API key, Anvil RPC URL), and cached metadata.
- Tables use versioned keys (`v1::<entity>::<hash>`) to ease upgrades.
- Implement compaction hooks and size limits to prevent unbounded growth when tracking hundreds of chains.
- Favorite toggles are persisted synchronously to `favorites_addresses` / `favorites_transactions` so UI state matches disk on restart.

## Data Sources
- Alloy provides RPC, tracing, and debug functionality; configure per-chain endpoints and retry policies.
- Local Anvil RPC is used to surface recent account activity; scan a bounded window of latest blocks for interactions involving the selected address.
- Etherscan (and equivalents) supply contract source and ABI; respect their rate limits and surface errors in-line.
- Optional providers (Tenderly, Blockscout) may supply richer debug data; abstract behind traits for future swaps.

## Hydration Strategy
- Trigger full hydration on selection but store timestamps; schedule refreshes via tokio tasks every N seconds.
- Use a shared loading state to inform UI panes and the top-section title indicator.
- Cache results in memory for instant tab switching; fall back to persistence if RPC is offline.

## Error Handling
- Distinguish between recoverable (rate limit) and fatal (schema mismatch) errors.
- Show errors in-context with actionable messaging and avoid panics.
- Log detailed causes via `tracing` for later inspection.

## Configuration
- Read `ETHERSCAN_API_KEY`, RPC URLs, and feature flags from environment or settings modal.
- Persist API secrets to the `secrets` partition so they survive restarts and can be overridden by environment variables when present.
- Detect missing configuration on startup and display an interactive secrets form modal before returning focus to the previous pane; keep the settings button badge warning in sync once the user supplies credentials.
