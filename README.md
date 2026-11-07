# Last Saver Standing

A no-loss Solana savings game. Everyone deposits the same amount into a round vault. Random check-in windows open. Miss one and you are out of the prize — your deposit still comes back. The last savers standing split a prize that is **not** taken from anyone's principal.

Built for the MLH Solana Hackathon, 6–8 November 2026. Devnet and local validator only.

## Pitch

Savings apps ask you to stay consistent. This one makes that the game. A round is a short battle royale inside a shared vault:

- Join by locking a fixed deposit.
- Survive randomly timed check-in windows.
- Spend a single **shield** to forgive one miss, or a single **jolt** to force a rival into one extra check-in.
- When the round ends, every deposit is withdrawable. Survivors also split the prize.

The prize is a sponsor bonus plus a published simulated APY on the pooled deposits. Both are paid from a separate reserve. The vault's accounting rejects any transfer that would dip into principal.

## How a round works

1. Someone initializes the arena once. That records the yield source as a mock reserve and the APY (default 5%).
2. Anyone can fund the reserve. That SOL is sponsor money, not a player deposit.
3. A host creates a round: entry amount, sponsor bonus, window length, gap range, and randomness mode.
4. Players join. The program transfers the entry into the round account and remembers it as principal.
5. The host starts the round. The sponsor bonus moves from the reserve into the prize bucket.
6. For each window, a crank commits a 32-byte seed, then reveals randomness:
   - **ORAO VRF** (devnet): the seed is committed only while the ORAO request account is still pending, so the crank cannot grind a known result. The fulfilled 64 bytes pick the gap.
   - **Demo slot hash**: the seed is mixed with the hash of the commit slot, which does not exist yet at commit time. This is commit-reveal, **not a VRF**. The account stores the mode so clients can say so.
7. The gap is `min_gap + (first 8 bytes as u64) % (max_gap - min_gap + 1)` seconds. The window then stays open for `window_duration`.
8. Required players check in. A shield can be armed beforehand; it is consumed only if you miss. Checking in keeps the charge.
9. A jolt, during a standard window, gives one rival one extra personal check-in after this window. Same duration. One jolt per player, one pending jolt per target. It does not move SOL.
10. After the window closes, anyone can resolve absences and finalize. The last standard window, or a round with one player left, settles.
11. Settlement quotes simulated yield for the seconds the principal was locked and pulls **at most** what the reserve can pay. Survivors split the prize. Remainder dust can be swept back to the reserve.
12. Anyone can crank `withdraw_principal` for any player, back to that player's wallet. Eliminated or not.

If a window cannot be opened (oracle stall, nobody cranking) past `progress_deadline`, anyone can refund the round. Prize money returns to the reserve. Deposits remain withdrawable.

## Architecture

```mermaid
flowchart LR
  subgraph wallets [Players]
    A[Wallet]
  end
  subgraph program [last_saver program]
    R[Round PDA<br/>principal + prize]
    Y[Yield reserve PDA<br/>sponsor funds]
  end
  A -->|join: fixed deposit| R
  Y -->|sponsor bonus + simulated yield| R
  R -->|principal back, always| A
  R -->|prize share| W[Last savers standing]
  program -->|ORAO mode| O[ORAO VRF account<br/>VRFzZoJdhFWL8rkvu87LpKM3RbcVezpMEc6X5GVDr7y]
  program -->|demo mode| S[SlotHashes sysvar]
```

One Anchor program, `last_saver`. Rounds and players are PDAs. The round account is the vault: lamports above rent are principal + prize + rounding dust, and every instruction that moves SOL checks that invariant before it succeeds.

The yield path is a single function, `accrue`. Today it computes

```text
principal * annual_bps * seconds / (10_000 * 31_536_000)
```

and transfers that from the reserve. A later lending or staking adapter would replace that function and still leave principal untouched. Rounds snapshot the rate and the source tag (`1` = mock reserve) at start, so an admin change does not rewrite a live round.

## Randomness

ORAO VRF classic is on devnet and mainnet at the same program id, `VRFzZoJdhFWL8rkvu87LpKM3RbcVezpMEc6X5GVDr7y` (crate `orao-solana-vrf` 0.8.x, updated November 2026). This build uses the classic account, not the callback program.

- Request PDA: `["orao-vrf-randomness-request", seed]`.
- The client submits ORAO `request_v2` and `commit_window_orao` in one transaction. Commit refuses a request that is already fulfilled.
- Reveal reads the fulfilled `RandomnessV2` account, checks the owner, the PDA, the seed, and the 64-byte value.

Demo mode is what the two-minute video and the program-test suite use, because it does not wait on the oracle. It is labelled `DemoSlot` on the account and in the UI. Slot leaders can bias future hashes. Do not use it for a prize you care about.

Switchboard On-Demand also exists on devnet (`Aio4gaXjXzJNVLtzwtNVmSqGKpANtXhybbkhtAC94ji2`). ORAO was chosen because the program id is shared with mainnet, the account layout is stable, and a round can verify it with a read instead of a quote-verification CPI.

## No-loss

- Join stores `entry_lamports` and adds that exact amount to `principal_lamports`.
- Prize credits only come from the reserve (sponsor bonus at start, simulated yield at settle).
- Withdrawals pay `entry_lamports` back to the player wallet and subtract it from principal.
- `assert_backing` reverts if the round's lamports would fall below rent + principal + prize + dust.
- Tests warp one year at 5% and expect exactly 0.15 SOL of yield on 3 SOL of principal, paid by the reserve. A second test funds the reserve with only the sponsor bonus and checks that yield stays 0 while both deposits still return in full.
- Cancel and stall-refund return the prize to the reserve and still allow principal withdrawals.

There is no admin instruction that withdraws principal.

## Program id

```text
52MzPzQgqVGXYZNRAbF1nivLSUh7NQgYAbhEoFE7NUyb
```

Deployed on **devnet** (SBF v2, 407,872 bytes). Upgrade authority is the deploy wallet and is not in git. The program keypair is not in git either (`target/deploy/` is ignored).

- [Program on Solana Explorer (devnet)](https://explorer.solana.com/address/52MzPzQgqVGXYZNRAbF1nivLSUh7NQgYAbhEoFE7NUyb?cluster=devnet)
- [Deploy transaction](https://explorer.solana.com/tx/NDzh7xG67CfwfDGsbQ3kRn2WSa9AwyndxEdJ2ANDNku1V8X9GgWwfAhnHNcVd6YNHfwHwmZcXL5GqKzjEpTkNTz?cluster=devnet)
- Program data account: [`956PxoM3MGfqA5jJoooDbiaYa8SaHTva8w6y7HaZbRuB`](https://explorer.solana.com/address/956PxoM3MGfqA5jJoooDbiaYa8SaHTva8w6y7HaZbRuB?cluster=devnet)

### Devnet smoke test

A two-wallet demo round (0.01 SOL entry, 0.01 SOL sponsor bonus, one 8-second window, demo slot-hash mode) was run against this deployment. Both players checked in, the round settled with 2 winners, each claimed 0.005 SOL, and both principal withdrawals succeeded. Bob’s wallet increased by exactly 0.010000 SOL. Mock yield on that short round was 0 lamports. Round account: [`D58RzD5Kt3ZxH2JgCAjT2L5RuawSwjrFUmMDy4XNYNPG`](https://explorer.solana.com/address/D58RzD5Kt3ZxH2JgCAjT2L5RuawSwjrFUmMDy4XNYNPG?cluster=devnet).

| Step | Transaction |
| --- | --- |
| Initialize arena | [5K5FXg…](https://explorer.solana.com/tx/5K5FXgFyth1TVqXKUCoFh4SS6VSvqo8mkeWnefucUEdt8QHbwMJNaAJgQnBUQ5Jc6KJk8oVmKKkDqNrir5ZUiYK3?cluster=devnet) |
| Fund reserve (0.02 SOL) | [R7zetE…](https://explorer.solana.com/tx/R7zetEDLW1yTLbELyy1xu7MbeHXBBjAkS1M75qZaLM9kC8KRzTvakwmZGb7egRhbeCTj7NQPzLMj6s5qsP9z2hi?cluster=devnet) |
| Create round 0 | [3vDrTE…](https://explorer.solana.com/tx/3vDrTEcyNH9AJwVqTLwLQHUZkFjT9v112FyPUET8mQBZsywTLEGkSK2NMobE8CHCNGvWNM6GJCcRxBTtrQquPtkf?cluster=devnet) |
| Join, wallet A | [3n8ARe…](https://explorer.solana.com/tx/3n8AResyfouAvvrGKbyAQ1wc4dwF51MWGj5a5hJMsdT1A2rFpRZG5ZsRBNcgJTb4fUAmsUpZCTeZXLohAvookCwf?cluster=devnet) |
| Join, wallet B | [iqgrJQ…](https://explorer.solana.com/tx/iqgrJQrRuUAMgxUnVY9mUzmfGtZqmH6bGceGkd4uFNA1W3vQCbb17H3PUVqr53GF8PH3TiSqKEMcvsgbawiotr6?cluster=devnet) |
| Check in A | [5WV9pW…](https://explorer.solana.com/tx/5WV9pWWy8SMt18vf5npEFVB37CcjR9EjWgiSoCDedzkcpLtmfQChPjufibh7RC1PPehUxYFSigriTHHXgTYxPAVj?cluster=devnet) |
| Check in B | [JRH6kX…](https://explorer.solana.com/tx/JRH6kXpMxZNnmjGB4z48bM3sH3VG3eL366XYb6RQtcggMgigoGnFwVoR6nzpvaSFB8baNW1q6VavWxJ1X8hFNM2?cluster=devnet) |
| Settle | [3aifZ7…](https://explorer.solana.com/tx/3aifZ7iB3MqtwhoyVPQ33Qe4AmcNjmHiJUpprhGdx8ZuMvMRQg3howCxYaqRuTeJLwLmMfrBf1cEbGZUFr3jeCjH?cluster=devnet) |
| Withdraw A | [2b1Uex…](https://explorer.solana.com/tx/2b1UexHxznCRwTPvxUntJsYQCgkfrt9hP9kFjVmdJBPCysEtdvoNZNGQXLutAYdiJVNb7QCsraK5WSvmMxYwJvQz?cluster=devnet) |
| Withdraw B | [4XKyuB…](https://explorer.solana.com/tx/4XKyuBqMmsSTEShABxHUW45QX73bcttkFVs4MH1NamroSrtGGYW3GMJQrvpSWJso5wSj1pQhr8Qa8x4LAvZ3zdPx?cluster=devnet) |

## Run the tests

Host toolchain: Rust 1.99 (see `rust-toolchain.toml`), Solana CLI 2.1.21, Anchor CLI 0.31.1. Tests run the program in-process with `solana-program-test`. They do not need a validator.

```bash
cargo test -p last_saver -- --test-threads=1
```

Covered: join, duplicate join, early start rejected, demo windows driven by a real slot hash, shield saved on check-in and consumed on a miss, elimination, jolt bounds, jolt elimination, one-year mock yield paid from the reserve, empty-reserve yield of zero, principal withdraw exact to the lamport, claim refused for eliminated players, double withdraw refused, ORAO commit rejected when the result is already known, ORAO reveal gap matches the fulfilled bytes, cancel, and stall refund.

## Frontend

```bash
cd app
npm install
npm run dev
```

Open http://127.0.0.1:5173. The default RPC is devnet (`https://api.devnet.solana.com`) and the default program id is the deployment above. For a local validator:

```bash
VITE_SOLANA_RPC=http://127.0.0.1:8899 VITE_SOLANA_CLUSTER=localnet npm run dev
```

Connect Phantom or Solflare, or use **Practice key** plus **Airdrop 2 SOL**. Fund the reserve, create a turbo round (15 second windows), join from a second wallet, start, draw the window, check in.

The UI recomputes the gap from the stored randomness and shows whether it matches. Demo rounds say they are not VRF. The prize card separates sponsor bonus from mock yield.

## Two-minute demo

```bash
cargo build-sbf --arch v2 -- -p last_saver
chmod +x scripts/demo-local.sh
./scripts/demo-local.sh
```

`cargo build-sbf` here is the Agave 4.4+ binary (platform-tools v1.57, `--arch v2`). Anchor 0.31's older platform-tools cannot read this lockfile, and a v3 ELF does not execute on the Agave 2.1 test validator.

That boots `solana-test-validator` with the program loaded and runs `app/scripts/turbo-demo.ts`:

- Three fresh keypairs, 0.05 SOL each, 0.05 SOL sponsor prize.
- 8 second windows, 4 second gap, demo slot-hash randomness.
- Alice checks in. Bob burns his shield by missing. Cara is eliminated.
- Alice jolts Bob. Both survive the second window. Bob misses the extra window and drops.
- Alice claims the prize. A separate crank pays fees so all three wallets increase by exactly 0.05 SOL of principal. Mock yield on a one-minute round is 0 lamports, which the script prints. The 0.05 SOL prize is the sponsor bonus.

Against an already running validator:

```bash
cd app && RPC=http://127.0.0.1:8899 npm run demo
```

## Deploy to devnet

Already deployed. To redeploy from a funded devnet wallet, with Agave 4.4+ on `PATH` so `cargo build-sbf --arch v2` exists:

```bash
solana config set --url https://api.devnet.solana.com
./scripts/devnet-deploy.sh
```

The script refuses a mainnet RPC. The public `requestAirdrop` faucet on `api.devnet.solana.com` is closed (`410`); use https://faucet.solana.com if you need more devnet SOL. Do not point this at mainnet and do not use real funds.

If a deploy stops midway, recover the buffer instead of leaving it funded:

```bash
solana program show --buffers --url https://api.devnet.solana.com
solana program close --buffers --url https://api.devnet.solana.com
```

## Hosting the frontend

The app is a static Vite build and can be hosted for free. GitHub Pages is the straightforward option. This repo does **not** enable Pages and does not change repository settings.

`.github/workflows/frontend.yml` builds `app/` when you start it by hand (Actions → Build frontend → Run workflow) and uploads `app/dist` as an artifact. It does not publish the site.

To turn that into GitHub Pages later, you do it yourself:

1. In `app/vite.config.ts`, set `base: "/last-saver-standing/"` so asset URLs match a project site.
2. Repository **Settings → Pages → Build and deployment → Source: GitHub Actions**.
3. Add a second job that uses `actions/deploy-pages` with `permissions: { pages: write, id-token: write }`. Until that setting is on, a deploy job fails, so it is not in this branch.

Any other static host works the same way: `cd app && npm ci && npm run build` and serve `app/dist`. The default RPC and program id are already devnet.

## Honest limitations

- Unaudited. A hackathon program, not a place to put money you cannot lose.
- Devnet and local validator only. No mainnet code path was exercised, and the deploy script bails if the RPC looks like mainnet.
- The visible prize on a short round is a sponsor bonus. The 5% figure is a simulated quote paid from that same reserve, not yield from lending or staking. The UI and the on-chain `yield_source` tag say so.
- Demo slot-hash mode is commit-reveal. It is not ORAO and not safe against a malicious slot leader.
- ORAO fulfillment depends on the devnet oracle. If it stalls, `refund_stalled` returns the prize to the reserve after the timeout so deposits are not stuck.
- Native SOL only. An SPL entry would be a second vault, behind the same principal accounting.
- Max 16 players. Windows need a crank; the UI and the demo script are that crank.
- The upgrade authority key is not published. The deployed program cannot be upgraded from this repo alone.
- Integer yield rounds down. A few minutes on a small deposit is often 0 lamports, which is the honest result.

## Layout

```text
programs/last_saver/   Anchor program and program-test suite
app/                   Vite + React wallet UI and turbo-demo.ts
scripts/demo-local.sh  local validator + demo
scripts/devnet-deploy.sh
.github/workflows/frontend.yml  manual frontend build, no Pages deploy
```
