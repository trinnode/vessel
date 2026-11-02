#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo build --manifest-path "$root/packages/contracts/Cargo.toml" --workspace \
  --target wasm32-unknown-unknown --release

mkdir -p "$root/packages/contracts/.assets"
for contract in nft wallet marketplace mock_verifier; do
  source="$root/packages/contracts/target/wasm32-unknown-unknown/release/${contract}.wasm"
  if [[ ! -f "$source" ]]; then
    echo "Missing compiled artifact: $source" >&2
    exit 1
  fi
  cp "$source" "$root/packages/contracts/.assets/${contract}.wasm"
done
