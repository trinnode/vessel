# Vessel — Shipping notes

## What to try

1. `pnpm install`
2. `pnpm --filter @vessel/web run dev`
3. Open `/` for the illustrated ownership loop, `/market` for the escrowed
   listing, and `/security` for the anti-drain model.
4. Run `pnpm --filter @vessel/sdk test` to validate safe RPC and deposit-link
   helpers.

## Honest production boundary

The UI deliberately labels demo data and does not pretend a wallet is
connected. Production launch requires Wallet Selector wiring, deployed
account IDs, and near-workspaces coverage against the release contracts.
Those are deployment inputs, not values that can safely be invented or
committed.
