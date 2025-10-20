# Data & Integrations Spec

## Persistence
- Fjall stores user data under `storage/`; create dedicated tables for addresses, transactions, settings, and cached metadata.
- Tables use versioned keys (`v1::<entity>::<hash>`) to ease upgrades.
- Implement compaction hooks and size limits to prevent unbounded growth when tracking hundreds of chains.
- Favorite toggles are persisted synchronously to `favorites_addresses` / `favorites_transactions` so UI state matches disk on restart.

## Data Sources
- Alloy provides RPC, tracing, and debug functionality; configure per-chain endpoints and retry policies.
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
- Detect missing configuration on startup and prompt the user via the settings button badge.
