# Vessel — Cross-Chain Portfolio NFT

**Pack a wallet. Sell it like an NFT.**

Vessel turns a multi-chain portfolio into one programmable NEAR NFT. Every
Vessel token is paired with a dedicated NEAR sub-account wallet. Assets can be
deposited through NEAR Intents, the portfolio can be listed as one product, and
ownership transfers through the NFT.

The critical security property is anti-drain escrow: once a Vessel is listed,
its wallet rejects withdrawals until the listing is cancelled or completed.
The seller cannot drain the portfolio between listing and settlement.

**Repository:** <https://github.com/trinnode/vessel>  
**License:** MIT  
**Hackathon track:** NFTs That Do Something

## The problem

Selling a portfolio across BTC, ETH, SOL, stablecoins, and NEAR normally means
many withdrawals, bridges, gas tokens, approvals, and counterparty steps.
Vessel makes the portfolio itself the asset being sold:

```text
source chains
     │
     └── NEAR Intents deposit ──> vessel-000001.vessel.near
                                      │
                                      └── owned by Vessel NFT #000001
                                                    │
                                      marketplace escrow locks withdrawals
                                                    │
                                      buyer receives NFT + wallet control
```

The NFT is not merely a receipt. It is the ownership key for the portfolio
wallet.

## Product flows

### Mint and pack

1. The user mints a Vessel NFT.
2. The factory creates a dedicated named sub-account.
3. The wallet records the NFT factory as its root of trust and the minter as
   its owner.
4. The user deposits supported assets through Intents, targeting that
   sub-account.
5. The gallery reads local holdings and verifier-held balances.

### List safely

1. The seller submits a price to the marketplace.
2. The marketplace asks the NFT factory to verify the seller and set the
   wallet's listed flag.
3. Only after the lock succeeds is the listing considered valid.
4. While listed, wallet withdrawals fail with the explicit
   `Vessel is listed: withdrawals locked` guard.

### Buy and hand off

1. The buyer attaches exactly the listing price.
2. The seller receives the price less the configured marketplace fee.
3. The marketplace transfers the listed NFT to the buyer.
4. The factory updates the wallet owner and unlocks withdrawals.
5. The buyer now controls the entire portfolio through the NFT.

## Workspace map

```text
packages/contracts/
  nft/             NFT factory, wallet provisioning, ownership, listing state
  wallet/          Per-vessel wallet, NEP-141 deposits, Intents withdrawals
  marketplace/     Fixed-price listings, escrow, exact payment, fee handling
  mock-verifier/   Sandbox-only verifier double
packages/sdk/      Typed RPC reads, explorer links, safe Intents URLs
packages/web/      Gallery, marketplace, and security model interface
scripts/           Release artifact build and explicit deployment guidance
.env.example       Public defaults and documented operator inputs
```

## Quick start

Requirements: Node.js 20+, pnpm 10+, Rust 1.95+, and a NEAR account only when
deploying or signing real transactions.

```bash
git clone https://github.com/trinnode/vessel
cd vessel
pnpm install
cp .env.example .env

# SDK tests and frontend checks
pnpm --filter @vessel/sdk test
pnpm --filter @vessel/web run typecheck
pnpm --filter @vessel/web run build

# Full Rust workspace
rustup run 1.95.0 cargo test --manifest-path packages/contracts/Cargo.toml --workspace

# Local UI
pnpm dev
```

The UI is safe to run in demo mode. It intentionally does not claim that a
wallet is connected or that a real contract is deployed until those inputs
are configured.

## Web experience

The Next.js app is designed as a premium vault interface:

- `/` — detailed ownership flow, featured vessels, and product narrative;
- `/market` — escrowed listing with price, holdings, and security badges;
- `/security` — anti-drain model and production boundary;
- `/` includes the Vessel identity, flow diagram, footer, and judge-oriented
  navigation.

The visual language uses deep navy, silver/platinum panels, cyan NEAR accents,
monospace balances, grid backgrounds, and the Vessel logo assets in
`packages/web/public/`.

## Contract architecture

### NFT factory

The NFT contract:

- creates a unique named sub-account per token;
- deploys the wallet code and initializes its owner;
- stores token metadata and wallet mapping;
- exposes token ownership and listing views;
- accepts marketplace-only lock, unlock, and transfer operations.

### Wallet

The wallet contract:

- accepts NEP-141 transfers through `ft_on_transfer`;
- tracks direct token balances;
- creates a verifier query for Intents-held assets;
- routes local withdrawals through `ft_transfer`;
- routes Intents withdrawals through the configured verifier;
- rejects all owner withdrawals while `is_listed` is true;
- accepts owner changes and listing state only from the NFT factory.

### Marketplace

The marketplace contract:

- validates non-zero prices;
- records seller, price, and block timestamp;
- requests the factory lock before retaining a listing;
- requires exact attached payment;
- calculates a bounded basis-point fee;
- pays the seller and transfers the NFT;
- allows only the seller to unlist.

## Security model

The primary threat is a seller listing a portfolio and draining it before a
buyer settles. Vessel addresses this at the wallet layer, not only in the UI:

```text
withdraw()
  ├─ caller must equal recorded NFT owner
  ├─ listed must be false
  ├─ amount must be positive
  └─ asset prefix must select a known withdrawal adapter
```

Additional invariants:

- a wallet cannot be initialized by an arbitrary account;
- marketplace access is restricted by the NFT factory;
- a seller cannot list another user's token;
- a buyer cannot underpay or overpay a listing;
- wallet ownership is updated when the NFT is handed off;
- verifier configuration is one-time per wallet;
- secrets and deployer keys never belong in the frontend or repository.

The mock verifier exists only for tests. It must never be deployed as
`intents.near` or used as a production custody contract.

## Configuration

```bash
cp .env.example .env
```

Public configuration includes the NEAR network/RPC, contract account IDs,
`intents.near`, CoinGecko base URL, and public Intents endpoints. Optional
CoinGecko API keys are documented in the template.

Deployment configuration includes the deployer account and fee receiver. The
actual key must live in the local NEAR credential store or hardware wallet.
Never commit `.env`, seed phrases, private keys, API JWTs, or deployer
credentials. Never expose a secret through `NEXT_PUBLIC_*`.

Before deposits are enabled, replace example account IDs with verified deployed
accounts and publish the exact WASM hashes.

## Testing

The current test layers are:

```bash
# Wallet, NFT, marketplace, and mock verifier unit tests
pnpm test:rust

# SDK URL validation and RPC helper tests
pnpm --filter @vessel/sdk test

# Frontend correctness
pnpm --filter @vessel/web run typecheck
pnpm --filter @vessel/web run build
```

The security-critical scenarios to keep in the release suite are:

1. mint creates a wallet owned by the minter;
2. a non-owner cannot withdraw;
3. listing freezes withdrawals;
4. unlisting restores withdrawals;
5. a failed lock does not leave a valid listing;
6. an exact-price buy transfers ownership;
7. wrong payment values are rejected;
8. a second verifier configuration is rejected;
9. wallet code cannot be initialized by a non-factory account.

The CI workflow also formats, tests, and builds all Rust contracts and runs
the frontend checks.

## Release and deployment sequence

The deployment script is deliberately explicit and refuses to guess account
IDs or submit transactions:

```bash
pnpm build:contracts
```

Then, on testnet:

1. deploy the wallet WASM or configure the factory with the exact code;
2. initialize the NFT factory with the wallet code and verifier account;
3. deploy and initialize the marketplace;
4. configure the marketplace account on the factory;
5. run mint, deposit, list, attempted withdrawal, unlist, and buy flows;
6. verify every deployed account and WASM hash;
7. switch the frontend to those account IDs;
8. repeat on mainnet with a hardware-backed deployer.

Do not treat the mock verifier or example account IDs as production values.
NEAR Intents verifier ABI changes must be reviewed before updating the adapter.

## Current production boundary

This repository contains the contract, SDK, and product foundations required
for shipping, but real custody requires deployment-specific inputs:

- verified NFT, marketplace, and verifier account IDs;
- Wallet Selector signing integration for the chosen wallet providers;
- a production Intents deposit/withdraw adapter;
- near-workspaces tests against the exact release WASM;
- an independent contract review before accepting valuable assets;
- monitoring, RPC fallback policy, and incident-response contacts.

Those values cannot be safely invented or committed to a public repository.
The UI and documentation call out the boundary instead of showing a
success-shaped fake transaction.

## Judge demo

```bash
pnpm install
pnpm --filter @vessel/sdk test
pnpm --filter @vessel/web run dev
```

Open `/` for the illustrated flow, `/market` for the escrowed portfolio
listing, and `/security` for the anti-drain explanation. The Rust suite is the
source of truth for the withdrawal lock; the visual warning is not the
security boundary.
