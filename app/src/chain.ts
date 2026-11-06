import { sha256 } from "@noble/hashes/sha256";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_SLOT_HASHES_PUBKEY,
  Transaction,
  TransactionInstruction,
  type Commitment,
} from "@solana/web3.js";

export const PROGRAM_ID = new PublicKey("52MzPzQgqVGXYZNRAbF1nivLSUh7NQgYAbhEoFE7NUyb");
export const ORAO_VRF_ID = new PublicKey("VRFzZoJdhFWL8rkvu87LpKM3RbcVezpMEc6X5GVDr7y");
export const LAMPORTS = 1_000_000_000;

const CONFIG_SEED = Buffer.from("config");
const YIELD_SEED = Buffer.from("yield");
const ROUND_SEED = Buffer.from("round");
const PLAYER_SEED = Buffer.from("player");
const ORAO_CONFIG_SEED = Buffer.from("orao-vrf-network-configuration");
const ORAO_REQUEST_SEED = Buffer.from("orao-vrf-randomness-request");
const REQUEST_V2 = Buffer.from([38, 151, 209, 6, 195, 102, 28, 217]);

export type WalletSigner = {
  publicKey: PublicKey;
  signTransaction(tx: Transaction): Promise<Transaction>;
};

export type RoundStatus =
  | "forming"
  | "needCommit"
  | "committed"
  | "open"
  | "settled"
  | "cancelled";

export type WindowKind = "none" | "standard" | "jolt";
export type RandomnessMode = "demoSlot" | "orao";

export type RoundView = {
  address: string;
  id: number;
  authority: string;
  entry: number;
  sponsorBonus: number;
  maxPlayers: number;
  minPlayers: number;
  playerCount: number;
  aliveCount: number;
  standardsScheduled: number;
  standardsOpened: number;
  windowDuration: number;
  minGap: number;
  maxGap: number;
  stallTimeout: number;
  joinUntil: number;
  startedAt: number;
  settledAt: number;
  progressDeadline: number;
  principal: number;
  prize: number;
  prizeDust: number;
  yieldAccrued: number;
  yieldBps: number;
  yieldSource: number;
  winners: number;
  prizePerWinner: number;
  prizeRefunded: boolean;
  status: RoundStatus;
  mode: RandomnessMode;
  pendingKind: WindowKind;
  currentKind: WindowKind;
  windowId: number;
  openAt: number;
  closeAt: number;
  attendeesRemaining: number;
  joltMarks: number;
  commitSlot: number;
  lastGap: number;
  commitSeed: Uint8Array;
  lastSeed: Uint8Array;
  lastRandomness: Uint8Array;
  players: string[];
};

export type PlayerView = {
  address: string;
  user: string;
  alive: boolean;
  shield: number;
  sabotage: number;
  shieldArmedFor: number;
  checkedWindow: number;
  joltPending: boolean;
  joltsReceived: number;
  withdrawn: boolean;
  claimed: boolean;
};

export type ConfigView = {
  admin: string;
  annualYieldBps: number;
  yieldSource: number;
  nextRoundId: number;
};

export type CreateRoundInput = {
  entry: number;
  sponsorBonus: number;
  maxPlayers: number;
  minPlayers: number;
  windows: number;
  windowDuration: number;
  minGap: number;
  maxGap: number;
  stallTimeout: number;
  joinUntil: number;
  mode: RandomnessMode;
};

export class TxFailed extends Error {
  logs: string[];
  constructor(message: string, logs: string[] = []) {
    super(message);
    this.logs = logs;
  }
}

export function keypairSigner(keypair: Keypair): WalletSigner {
  return {
    publicKey: keypair.publicKey,
    async signTransaction(tx: Transaction) {
      tx.partialSign(keypair);
      return tx;
    },
  };
}

export function configPda() {
  return PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID)[0];
}

export function reservePda() {
  return PublicKey.findProgramAddressSync([YIELD_SEED], PROGRAM_ID)[0];
}

export function roundPda(id: number) {
  return PublicKey.findProgramAddressSync([ROUND_SEED, u64(id)], PROGRAM_ID)[0];
}

export function playerPda(round: PublicKey, user: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [PLAYER_SEED, round.toBuffer(), user.toBuffer()],
    PROGRAM_ID,
  )[0];
}

export function oraoConfigPda() {
  return PublicKey.findProgramAddressSync([ORAO_CONFIG_SEED], ORAO_VRF_ID)[0];
}

export function oraoRequestPda(seed: Uint8Array) {
  return PublicKey.findProgramAddressSync([ORAO_REQUEST_SEED, Buffer.from(seed)], ORAO_VRF_ID)[0];
}

export function gapFromRandomness(randomness: Uint8Array, minGap: number, maxGap: number) {
  if (maxGap <= minGap) return minGap;
  let sample = 0n;
  for (let i = 0; i < 8; i += 1) sample |= BigInt(randomness[i] ?? 0) << BigInt(8 * i);
  const span = BigInt(maxGap - minGap + 1);
  return minGap + Number(sample % span);
}

export function shortKey(value: string) {
  return value.length < 10 ? value : `${value.slice(0, 4)}…${value.slice(-4)}`;
}

export function formatSol(lamports: number) {
  return (lamports / LAMPORTS).toLocaleString(undefined, {
    minimumFractionDigits: 4,
    maximumFractionDigits: 4,
  });
}

export async function loadConfig(connection: Connection): Promise<ConfigView | null> {
  const info = await connection.getAccountInfo(configPda());
  if (!info) return null;
  const data = info.data.subarray(8);
  const admin = new PublicKey(data.subarray(0, 32)).toBase58();
  const annualYieldBps = data.readUInt16LE(32);
  const yieldSource = data.readUInt8(34);
  const nextRoundId = Number(data.readBigUInt64LE(35));
  return { admin, annualYieldBps, yieldSource, nextRoundId };
}

export async function loadRound(connection: Connection, id: number): Promise<RoundView | null> {
  const address = roundPda(id);
  const info = await connection.getAccountInfo(address);
  if (!info) return null;
  return decodeRound(address.toBase58(), info.data);
}

export async function loadPlayer(
  connection: Connection,
  round: PublicKey,
  user: PublicKey,
): Promise<PlayerView | null> {
  const address = playerPda(round, user);
  const info = await connection.getAccountInfo(address);
  if (!info) return null;
  const data = info.data.subarray(8);
  return {
    address: address.toBase58(),
    user: new PublicKey(data.subarray(32, 64)).toBase58(),
    alive: data.readUInt8(64) === 1,
    shield: data.readUInt8(65),
    sabotage: data.readUInt8(66),
    shieldArmedFor: data.readUInt16LE(67),
    checkedWindow: data.readUInt16LE(69),
    joltPending: data.readUInt8(73) === 1,
    joltsReceived: data.readUInt8(74),
    withdrawn: data.readUInt8(75) === 1,
    claimed: data.readUInt8(76) === 1,
  };
}

export async function send(
  connection: Connection,
  signer: WalletSigner,
  instructions: TransactionInstruction[],
  commitment: Commitment = "confirmed",
) {
  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash(commitment);
  const tx = new Transaction({
    feePayer: signer.publicKey,
    blockhash,
    lastValidBlockHeight,
  }).add(...instructions);
  const signed = await signer.signTransaction(tx);
  try {
    const signature = await connection.sendRawTransaction(signed.serialize(), {
      skipPreflight: false,
    });
    await connection.confirmTransaction({ signature, blockhash, lastValidBlockHeight }, commitment);
    return signature;
  } catch (error) {
    const logs = extractLogs(error);
    const anchor = logs.find((line) => line.includes("Error Message:"));
    throw new TxFailed(anchor ?? (error instanceof Error ? error.message : "Transaction failed"), logs);
  }
}

export async function initialize(connection: Connection, signer: WalletSigner, annualYieldBps = 500) {
  return send(connection, signer, [
    ix("initialize", u16(annualYieldBps), [
      signerMeta(signer.publicKey, true),
      meta(configPda(), true),
      meta(reservePda(), true),
      meta(SystemProgram.programId, false),
    ]),
  ]);
}

export async function fundReserve(connection: Connection, signer: WalletSigner, lamports: number) {
  return send(connection, signer, [
    ix("fund_reserve", u64(lamports), [
      signerMeta(signer.publicKey, true),
      meta(reservePda(), true),
      meta(SystemProgram.programId, false),
    ]),
  ]);
}

export async function createRound(
  connection: Connection,
  signer: WalletSigner,
  id: number,
  input: CreateRoundInput,
) {
  const data = Buffer.concat([
    u64(id),
    u64(input.entry),
    u64(input.sponsorBonus),
    u8(input.maxPlayers),
    u8(input.minPlayers),
    u8(input.windows),
    i64(input.windowDuration),
    i64(input.minGap),
    i64(input.maxGap),
    i64(input.stallTimeout),
    i64(input.joinUntil),
    u8(input.mode === "orao" ? 1 : 0),
  ]);
  return send(connection, signer, [
    ix("create_round", data, [
      signerMeta(signer.publicKey, true),
      meta(configPda(), true),
      meta(roundPda(id), true),
      meta(SystemProgram.programId, false),
    ]),
  ]);
}

export async function joinRound(connection: Connection, signer: WalletSigner, id: number) {
  const round = roundPda(id);
  return send(connection, signer, [
    ix("join", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(round, true),
      meta(playerPda(round, signer.publicKey), true),
      meta(SystemProgram.programId, false),
    ]),
  ]);
}

export async function startRound(connection: Connection, signer: WalletSigner, id: number) {
  return send(connection, signer, [
    ix("start_round", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(configPda(), false),
      meta(roundPda(id), true),
      meta(reservePda(), true),
    ]),
  ]);
}

export async function commitDemo(connection: Connection, signer: WalletSigner, id: number, seed: Uint8Array) {
  return send(connection, signer, [
    ix("commit_window", Buffer.from(seed), [
      signerMeta(signer.publicKey, true),
      meta(roundPda(id), true),
    ]),
  ]);
}

export async function revealDemo(connection: Connection, signer: WalletSigner, id: number) {
  return send(connection, signer, [
    ix("reveal_window_demo", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(roundPda(id), true),
      meta(SYSVAR_SLOT_HASHES_PUBKEY, false),
    ]),
  ]);
}

export async function drawDemoWindow(connection: Connection, signer: WalletSigner, id: number) {
  const seed = randomSeed();
  await commitDemo(connection, signer, id, seed);
  const committed = await loadRound(connection, id);
  if (!committed) throw new Error("Round disappeared after commit");
  const deadline = Date.now() + 20_000;
  let revealed = false;
  while (Date.now() < deadline) {
    const slot = await connection.getSlot("confirmed");
    if (slot > committed.commitSlot) {
      try {
        await revealDemo(connection, signer, id);
        revealed = true;
        break;
      } catch (error) {
        if (!(error instanceof TxFailed) || !error.message.includes("Slot hash")) throw error;
      }
    }
    await sleep(400);
  }
  if (!revealed) throw new Error("Slot hash was not ready in time. Try Reveal again.");
  return seed;
}

export async function commitAndRequestOrao(
  connection: Connection,
  signer: WalletSigner,
  id: number,
  seed: Uint8Array,
) {
  const treasury = await oraoTreasury(connection);
  const request = oraoRequestPda(seed);
  return send(connection, signer, [
    new TransactionInstruction({
      programId: ORAO_VRF_ID,
      keys: [
        signerMeta(signer.publicKey, true),
        meta(oraoConfigPda(), true),
        meta(treasury, true),
        meta(request, true),
        meta(SystemProgram.programId, false),
      ],
      data: Buffer.concat([REQUEST_V2, Buffer.from(seed)]),
    }),
    ix("commit_window_orao", Buffer.from(seed), [
      signerMeta(signer.publicKey, true),
      meta(roundPda(id), true),
      meta(request, false),
    ]),
  ]);
}

export async function revealOrao(connection: Connection, signer: WalletSigner, id: number, seed: Uint8Array) {
  return send(connection, signer, [
    ix("reveal_window_orao", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(roundPda(id), true),
      meta(oraoRequestPda(seed), false),
    ]),
  ]);
}

export async function drawOraoWindow(connection: Connection, signer: WalletSigner, id: number) {
  const seed = randomSeed();
  await commitAndRequestOrao(connection, signer, id, seed);
  const request = oraoRequestPda(seed);
  const deadline = Date.now() + 45_000;
  while (Date.now() < deadline) {
    const info = await connection.getAccountInfo(request, "confirmed");
    if (info && info.data.length >= 137 && info.data[8] === 1) {
      await revealOrao(connection, signer, id, seed);
      return seed;
    }
    await sleep(1000);
  }
  throw new Error("ORAO did not fulfill the request in time. The round can still be refunded if it stalls.");
}

export async function checkIn(connection: Connection, signer: WalletSigner, id: number) {
  const round = roundPda(id);
  return send(connection, signer, [
    ix("check_in", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(round, true),
      meta(playerPda(round, signer.publicKey), true),
    ]),
  ]);
}

export async function armShield(connection: Connection, signer: WalletSigner, id: number) {
  const round = roundPda(id);
  return send(connection, signer, [
    ix("arm_shield", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(round, true),
      meta(playerPda(round, signer.publicKey), true),
    ]),
  ]);
}

export async function sabotage(
  connection: Connection,
  signer: WalletSigner,
  id: number,
  target: PublicKey,
) {
  const round = roundPda(id);
  return send(connection, signer, [
    ix("sabotage", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(target, false),
      meta(round, true),
      meta(playerPda(round, signer.publicKey), true),
      meta(playerPda(round, target), true),
    ]),
  ]);
}

export async function resolveAbsence(
  connection: Connection,
  signer: WalletSigner,
  id: number,
  user: PublicKey,
) {
  const round = roundPda(id);
  return send(connection, signer, [
    ix("resolve_absence", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(user, false),
      meta(round, true),
      meta(playerPda(round, user), true),
    ]),
  ]);
}

export async function finalizeWindow(connection: Connection, signer: WalletSigner, id: number) {
  return send(connection, signer, [
    ix("finalize_window", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(roundPda(id), true),
      meta(reservePda(), true),
    ]),
  ]);
}

export async function withdrawPrincipal(
  connection: Connection,
  signer: WalletSigner,
  id: number,
  user: PublicKey,
) {
  const round = roundPda(id);
  return send(connection, signer, [
    ix("withdraw_principal", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(user, true),
      meta(round, true),
      meta(playerPda(round, user), true),
    ]),
  ]);
}

export async function claimPrize(connection: Connection, signer: WalletSigner, id: number, user: PublicKey) {
  const round = roundPda(id);
  return send(connection, signer, [
    ix("claim_prize", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(user, true),
      meta(round, true),
      meta(playerPda(round, user), true),
    ]),
  ]);
}

export async function refundStalled(connection: Connection, signer: WalletSigner, id: number) {
  return send(connection, signer, [
    ix("refund_stalled", Buffer.alloc(0), [
      signerMeta(signer.publicKey, true),
      meta(roundPda(id), true),
      meta(reservePda(), true),
    ]),
  ]);
}

export function mustResolve(round: RoundView, player: PlayerView, now: number) {
  if (round.status !== "open" || now < round.closeAt || !player.alive) return false;
  if (player.checkedWindow === round.windowId) return false;
  if (round.currentKind === "jolt") return player.joltPending;
  return round.currentKind === "standard";
}

export function randomSeed() {
  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  if (seed.every((byte) => byte === 0)) seed[0] = 1;
  return seed;
}

async function oraoTreasury(connection: Connection) {
  const info = await connection.getAccountInfo(oraoConfigPda());
  if (!info || info.data.length < 72) {
    throw new Error(
      "ORAO VRF config was not found on this cluster. Use a demo-slot round, or point the RPC at devnet.",
    );
  }
  return new PublicKey(info.data.subarray(40, 72));
}

function decodeRound(address: string, raw: Buffer): RoundView {
  const data = raw.subarray(8);
  let offset = 0;
  const id = readU64(data, offset);
  offset += 8;
  const authority = readKey(data, offset);
  offset += 32;
  const entry = readU64(data, offset);
  offset += 8;
  const sponsorBonus = readU64(data, offset);
  offset += 8;
  const maxPlayers = data.readUInt8(offset);
  offset += 1;
  const minPlayers = data.readUInt8(offset);
  offset += 1;
  const playerCount = data.readUInt8(offset);
  offset += 1;
  const aliveCount = data.readUInt8(offset);
  offset += 1;
  const standardsScheduled = data.readUInt8(offset);
  offset += 1;
  const standardsOpened = data.readUInt8(offset);
  offset += 1;
  const windowDuration = readI64(data, offset);
  offset += 8;
  const minGap = readI64(data, offset);
  offset += 8;
  const maxGap = readI64(data, offset);
  offset += 8;
  const stallTimeout = readI64(data, offset);
  offset += 8;
  const joinUntil = readI64(data, offset);
  offset += 8;
  const startedAt = readI64(data, offset);
  offset += 8;
  offset += 8;
  const settledAt = readI64(data, offset);
  offset += 8;
  const progressDeadline = readI64(data, offset);
  offset += 8;
  const principal = readU64(data, offset);
  offset += 8;
  const prize = readU64(data, offset);
  offset += 8;
  const prizeDust = readU64(data, offset);
  offset += 8;
  const yieldAccrued = readU64(data, offset);
  offset += 8;
  const yieldBps = data.readUInt16LE(offset);
  offset += 2;
  const yieldSource = data.readUInt8(offset);
  offset += 1;
  const winners = data.readUInt8(offset);
  offset += 1;
  const prizePerWinner = readU64(data, offset);
  offset += 8;
  const prizeRefunded = data.readUInt8(offset) === 1;
  offset += 1;
  const status = statusName(data.readUInt8(offset));
  offset += 1;
  const mode: RandomnessMode = data.readUInt8(offset) === 1 ? "orao" : "demoSlot";
  offset += 1;
  const pendingKind = kindName(data.readUInt8(offset));
  offset += 1;
  const currentKind = kindName(data.readUInt8(offset));
  offset += 1;
  const windowId = data.readUInt16LE(offset);
  offset += 2;
  const openAt = readI64(data, offset);
  offset += 8;
  const closeAt = readI64(data, offset);
  offset += 8;
  const attendeesRemaining = data.readUInt8(offset);
  offset += 1;
  const joltMarks = data.readUInt8(offset);
  offset += 1;
  const commitSlot = readU64(data, offset);
  offset += 8;
  offset += 8;
  const lastGap = readI64(data, offset);
  offset += 8;
  const commitSeed = data.subarray(offset, offset + 32);
  offset += 32;
  const lastSeed = data.subarray(offset, offset + 32);
  offset += 32;
  const lastRandomness = data.subarray(offset, offset + 64);
  offset += 64;
  offset += 1;
  const players: string[] = [];
  for (let i = 0; i < playerCount; i += 1) {
    players.push(readKey(data, offset + i * 32));
  }
  return {
    address,
    id,
    authority,
    entry,
    sponsorBonus,
    maxPlayers,
    minPlayers,
    playerCount,
    aliveCount,
    standardsScheduled,
    standardsOpened,
    windowDuration,
    minGap,
    maxGap,
    stallTimeout,
    joinUntil,
    startedAt,
    settledAt,
    progressDeadline,
    principal,
    prize,
    prizeDust,
    yieldAccrued,
    yieldBps,
    yieldSource,
    winners,
    prizePerWinner,
    prizeRefunded,
    status,
    mode,
    pendingKind,
    currentKind,
    windowId,
    openAt,
    closeAt,
    attendeesRemaining,
    joltMarks,
    commitSlot,
    lastGap,
    commitSeed,
    lastSeed,
    lastRandomness,
    players,
  };
}

function statusName(value: number): RoundStatus {
  return (["forming", "needCommit", "committed", "open", "settled", "cancelled"] as const)[value] ?? "forming";
}

function kindName(value: number): WindowKind {
  return (["none", "standard", "jolt"] as const)[value] ?? "none";
}

function ix(name: string, data: Buffer, keys: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[]) {
  return new TransactionInstruction({
    programId: PROGRAM_ID,
    keys,
    data: Buffer.concat([discriminator(name), data]),
  });
}

function discriminator(name: string) {
  return Buffer.from(sha256(new TextEncoder().encode(`global:${name}`))).subarray(0, 8);
}

function meta(pubkey: PublicKey, isWritable: boolean) {
  return { pubkey, isSigner: false, isWritable };
}

function signerMeta(pubkey: PublicKey, isWritable: boolean) {
  return { pubkey, isSigner: true, isWritable };
}

function u64(value: number) {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(BigInt(value));
  return buffer;
}

function i64(value: number) {
  const buffer = Buffer.alloc(8);
  buffer.writeBigInt64LE(BigInt(value));
  return buffer;
}

function u16(value: number) {
  const buffer = Buffer.alloc(2);
  buffer.writeUInt16LE(value);
  return buffer;
}

function u8(value: number) {
  return Buffer.from([value]);
}

function readU64(data: Buffer, offset: number) {
  return Number(data.readBigUInt64LE(offset));
}

function readI64(data: Buffer, offset: number) {
  return Number(data.readBigInt64LE(offset));
}

function readKey(data: Buffer, offset: number) {
  return new PublicKey(data.subarray(offset, offset + 32)).toBase58();
}

function extractLogs(error: unknown) {
  if (!error || typeof error !== "object") return [];
  const logs = (error as { logs?: string[] }).logs;
  if (Array.isArray(logs)) return logs;
  const nested = (error as { transactionLogs?: string[] }).transactionLogs;
  return Array.isArray(nested) ? nested : [];
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
