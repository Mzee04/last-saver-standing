#!/usr/bin/env bash
# Boot a local validator with the program and play a full turbo round.
set -euo pipefail
cd "$(dirname "$0")/.."
# Prefer Agave 4.4 when it is installed. Its cargo-build-sbf produces the ELF,
# and its test validator can execute that ELF. The 2.1.21 validator cannot.
if [[ -x "${HOME}/.local/share/solana/install/releases/4.4.1/solana-release/bin/solana" ]]; then
  export PATH="${HOME}/.local/share/solana/install/releases/4.4.1/solana-release/bin:${PATH}"
else
  export PATH="${HOME}/.local/share/solana/install/active_release/bin:${PATH}"
fi
ID="52MzPzQgqVGXYZNRAbF1nivLSUh7NQgYAbhEoFE7NUyb"
SO="target/deploy/last_saver.so"
if [[ ! -f "$SO" ]]; then
  echo "Missing $SO. Run: anchor build" >&2
  exit 1
fi
if ! solana-test-validator --help >/dev/null 2>&1; then
  echo "solana-test-validator is not on PATH." >&2
  exit 1
fi
pkill -f "solana-test-validator" >/dev/null 2>&1 || true
sleep 1
rm -rf /tmp/lss-ledger
solana-test-validator --reset --quiet --ledger /tmp/lss-ledger --bpf-program "$ID" "$SO" >/tmp/last-saver-validator.log 2>&1 &
VALIDATOR=$!
cleanup() { kill "$VALIDATOR" >/dev/null 2>&1 || true; }
trap cleanup EXIT
ready=0
for _ in $(seq 1 50); do
  if solana -u localhost account "$ID" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 0.4
done
if [[ "$ready" != 1 ]]; then
  echo "Validator did not load $ID. See /tmp/last-saver-validator.log" >&2
  exit 1
fi
cd app
RPC=http://127.0.0.1:8899 npm run demo
