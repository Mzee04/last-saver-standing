/**
 * Two-minute local or devnet demo.
 *
 *   solana-test-validator --reset --bpf-program 52MzPzQgqVGXYZNRAbF1nivLSUh7NQgYAbhEoFE7NUyb \
 *     ../target/deploy/last_saver.so
 *   npm run demo
 *
 * RPC defaults to http://127.0.0.1:8899. Set RPC=https://api.devnet.solana.com for devnet.
 * The script uses demo slot-hash windows, not ORAO, so it does not wait on the oracle.
 */
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey } from "@solana/web3.js";
import {
  LAMPORTS,
  checkIn,
  claimPrize,
  createRound,
  drawDemoWindow,
  finalizeWindow,
  fundReserve,
  initialize,
  joinRound,
  keypairSigner,
  loadConfig,
  loadPlayer,
  loadRound,
  resolveAbsence,
  roundPda,
  sabotage,
  armShield,
  startRound,
  withdrawPrincipal,
  type RoundView,
  type WalletSigner,
} from "../src/chain";

const rpc = process.env.RPC ?? "http://127.0.0.1:8899";
const connection = new Connection(rpc, "confirmed");

async function main() {
  const alice = Keypair.generate();
  const bob = Keypair.generate();
  const cara = Keypair.generate();
  const crank = Keypair.generate();
  const cast = [
    ["Alice", alice],
    ["Bob", bob],
    ["Cara", cara],
  ] as const;
  console.log(`\nLast Saver Standing — turbo demo\nRPC ${rpc}\n`);
  for (const [name, key] of cast) {
    await airdrop(key.publicKey, 2);
    console.log(`${name} ${key.publicKey.toBase58()}`);
  }
  await airdrop(crank.publicKey, 1);
  const payer = keypairSigner(crank);

  const host = keypairSigner(alice);
  if (!(await loadConfig(connection))) {
    await initialize(connection, host, 500);
    console.log("Arena initialized. Yield source = mock reserve at 5% APY.");
  }
  await fundReserve(connection, host, Math.round(0.2 * LAMPORTS));
  const config = await loadConfig(connection);
  if (!config) throw new Error("Config missing after initialize");
  const id = config.nextRoundId;
  const now = Math.floor(Date.now() / 1000);
  await createRound(connection, host, id, {
    entry: Math.round(0.05 * LAMPORTS),
    sponsorBonus: Math.round(0.05 * LAMPORTS),
    maxPlayers: 8,
    minPlayers: 3,
    windows: 2,
    windowDuration: 8,
    minGap: 4,
    maxGap: 4,
    stallTimeout: 180,
    joinUntil: now + 300,
    mode: "demoSlot",
  });
  console.log(`\nRound ${id}: 0.05 SOL deposit, 0.05 SOL sponsor prize, 8s windows, 4s gap.`);
  console.log("Randomness is demo slot-hash commit-reveal, not ORAO VRF.\n");

  for (const [, key] of cast) await joinRound(connection, keypairSigner(key), id);
  await startRound(connection, host, id);
  console.log("All three locked their deposit. The prize was moved out of the sponsor reserve.");

  await playWindow(id, host, "Window 1", async (round) => {
    await checkIn(connection, host, id);
    await armShield(connection, keypairSigner(bob), id);
    console.log("Alice checks in. Bob arms a shield and misses. Cara misses with no shield.");
    await sleepUntil(round.closeAt + 2);
    await resolveAbsence(connection, host, id, bob.publicKey);
    await resolveAbsence(connection, host, id, cara.publicKey);
    await finalizeWindow(connection, host, id);
  });

  await playWindow(id, host, "Window 2", async (round) => {
    await sabotage(connection, host, id, bob.publicKey);
    await checkIn(connection, host, id);
    await checkIn(connection, keypairSigner(bob), id);
    console.log("Alice jolts Bob, then both check in. Cara is already out.");
    await sleepUntil(round.closeAt + 2);
    await finalizeWindow(connection, host, id);
  });

  await playWindow(id, host, "Jolt window", async (round) => {
    console.log("Only Bob must attend. He misses, and the shield was already used.");
    await sleepUntil(round.closeAt + 2);
    await resolveAbsence(connection, host, id, bob.publicKey);
    await finalizeWindow(connection, host, id);
  });

  const settled = await loadRound(connection, id);
  if (!settled || settled.status !== "settled") throw new Error(`Expected settlement, saw ${settled?.status}`);
  console.log(
    `\nSettled. Winners: ${settled.winners}. Prize each: ${sol(settled.prizePerWinner)} SOL. Mock yield booked: ${sol(settled.yieldAccrued)} SOL.`,
  );
  await claimPrize(connection, payer, id, alice.publicKey);
  for (const [name, key] of cast) {
    const before = await connection.getBalance(key.publicKey);
    await withdrawPrincipal(connection, payer, id, key.publicKey);
    const after = await connection.getBalance(key.publicKey);
    const player = await loadPlayer(connection, roundPda(id), key.publicKey);
    console.log(
      `${name} deposit returned ${(after - before) / LAMPORTS_PER_SOL} SOL ` +
        `(alive=${player?.alive}, withdrawn=${player?.withdrawn}).`,
    );
  }
  const vault = await connection.getBalance(roundPda(id));
  console.log(`\nRound vault still holds ${sol(vault)} SOL of account rent, not player principal.`);
  console.log("Demo complete.\n");
}

async function playWindow(
  id: number,
  host: WalletSigner,
  label: string,
  act: (round: RoundView) => Promise<void>,
) {
  console.log(`\n— ${label}`);
  await drawDemoWindow(connection, host, id);
  const round = await loadRound(connection, id);
  if (!round) throw new Error("missing round");
  console.log(`Opens in ${Math.max(0, round.openAt - Math.floor(Date.now() / 1000))}s, gap ${round.lastGap}s.`);
  // The validator clock can lag wall time by a second. Land inside the window.
  await sleepUntil(round.openAt + 2);
  await act(round);
}

async function airdrop(pubkey: PublicKey, solAmount: number) {
  const signature = await connection.requestAirdrop(pubkey, solAmount * LAMPORTS_PER_SOL);
  const latest = await connection.getLatestBlockhash();
  await connection.confirmTransaction({ signature, ...latest }, "confirmed");
}

async function sleepUntil(unixSeconds: number) {
  const ms = unixSeconds * 1000 - Date.now();
  if (ms > 0) await new Promise((resolve) => setTimeout(resolve, ms));
}

function sol(lamports: number) {
  return (lamports / LAMPORTS_PER_SOL).toFixed(4);
}

main().catch((error: unknown) => {
  console.error(error);
  process.exit(1);
});
