#!/usr/bin/env bash
set -euo pipefail

cat >&2 <<'EOF'
Deployment is intentionally explicit. Set DEPLOYER_ACCOUNT_ID and contract
account IDs in a local .env, build the release artifacts, deploy to testnet,
and run the sandbox/security checklist before enabling mainnet.

Suggested flow:
  pnpm build:contracts
  cargo near deploy <account> --network testnet

This script will not guess account IDs or submit transactions.
EOF
exit 2
