# Vessel

Vessel is a NEAR portfolio NFT: each token owns a dedicated sub-account, and
the marketplace escrows the token while listed so the portfolio cannot be
drained before settlement.

## Workspace

- `packages/contracts/nft`: NEP-171-style factory, token ownership, and wallet
  provisioning.
- `packages/contracts/wallet`: per-vessel wallet with local NEP-141 deposits,
  Intents withdrawal adapter, and the anti-drain listing guard.
- `packages/contracts/marketplace`: fixed-price listings, escrow state, fee
  accounting, and exact-payment checks.
- `packages/sdk`: typed read-only NEAR RPC and Intents URL helpers.
- `packages/web`: gallery, marketplace, and security model interface.

## Local development

```sh
pnpm install
cp .env.example .env
pnpm --filter @vessel/sdk test
pnpm --filter @vessel/web run typecheck
pnpm --filter @vessel/web run build
pnpm test:rust
```

The web app is safe to run in demo mode with `NEXT_PUBLIC_DEMO_MODE=1`. Before
using real assets, replace the example contract IDs with deployed accounts and
complete the sandbox suite against the exact release WASM.

## Deployment checklist

1. Install a supported stable Rust toolchain and `cargo-near`.
2. Deploy the wallet code and initialize the NFT factory on testnet.
3. Configure the marketplace account on the factory.
4. Run mint, deposit, list, attempted withdrawal, unlist, and buy tests.
5. Pin the real Intents verifier account and publish the contract hashes.
6. Only then repeat deployment on mainnet with a hardware-backed deployer.

No secret belongs in `.env.example`, CI logs, source control, or the frontend.
