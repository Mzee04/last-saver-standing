#!/usr/bin/env bash
# Deploy Last Saver Standing to devnet. Never points at mainnet.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="${HOME}/.local/share/solana/install/active_release/bin:${HOME}/.local/bin:${PATH}"
solana config set --url devnet >/dev/null
echo "RPC: $(solana config get | awk '/RPC URL/{print $3}')"
if solana config get | grep -q mainnet; then
  echo "Refusing to deploy: RPC is mainnet." >&2
  exit 1
fi
KEY="target/deploy/last_saver-keypair.json"
if [[ ! -f "$KEY" ]]; then
  mkdir -p target/deploy
  solana-keygen new -o "$KEY" --no-bip39-passphrase -f
  echo "New program id: $(solana-keygen pubkey "$KEY")"
  echo "Update declare_id! and Anchor.toml before building." >&2
  exit 1
fi
# Anchor 0.31's bundled platform-tools cannot parse this lockfile.
# Agave 4.4+ `cargo build-sbf` (Rust 1.95 / platform-tools v1.57) can.
cargo build-sbf --arch v2 -- -p last_saver
ID="$(solana-keygen pubkey "$KEY")"
echo "Program id $ID"
# Devnet airdrops are often rate-limited. Retry, then stop with the exact next step.
if ! solana balance | grep -q '[1-9]'; then
  for _ in 1 2 3 4; do
    solana airdrop 2 && break
    sleep 5
  done
fi
solana program deploy target/deploy/last_saver.so --program-id "$KEY" --url devnet
echo "Deployed $ID to devnet."
echo "Upgrade authority is the local solana wallet ($(solana address)). It is not committed."
