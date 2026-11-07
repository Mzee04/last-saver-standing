import { useWallet } from "@solana/wallet-adapter-react";
import { WalletMultiButton } from "@solana/wallet-adapter-react-ui";
import { useConnection } from "@solana/wallet-adapter-react";
import { Keypair, PublicKey } from "@solana/web3.js";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  LAMPORTS,
  PROGRAM_ID,
  TxFailed,
  armShield,
  checkIn,
  claimPrize,
  createRound,
  drawDemoWindow,
  drawOraoWindow,
  finalizeWindow,
  revealDemo,
  formatSol,
  fundReserve,
  gapFromRandomness,
  initialize,
  joinRound,
  keypairSigner,
  loadConfig,
  loadPlayer,
  loadRound,
  mustResolve,
  reservePda,
  refundStalled,
  resolveAbsence,
  sabotage,
  shortKey,
  startRound,
  withdrawPrincipal,
  type ConfigView,
  type PlayerView,
  type RoundView,
  type WalletSigner,
} from "./chain";

const ROUND_KEY = "lss.roundId";
const PRACTICE_KEY = "lss.practice";
const NAMES_KEY = "lss.names";

function readStoredPractice(): Keypair | null {
  try {
    const raw = sessionStorage.getItem(PRACTICE_KEY);
    if (!raw) return null;
    return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(raw) as number[]));
  } catch {
    return null;
  }
}

function rememberPractice(keypair: Keypair) {
  sessionStorage.setItem(PRACTICE_KEY, JSON.stringify(Array.from(keypair.secretKey)));
}

function displayName(user: string) {
  try {
    const names = JSON.parse(sessionStorage.getItem(NAMES_KEY) || "{}") as Record<string, string>;
    if (names[user]) return names[user];
  } catch {
    /* unnamed wallet */
  }
  return shortKey(user);
}

export function App({ endpoint, cluster }: { endpoint: string; cluster: string }) {
  const { connection } = useConnection();
  const wallet = useWallet();
  const [practice, setPractice] = useState<Keypair | null>(() => readStoredPractice());
  const [config, setConfig] = useState<ConfigView | null>(null);
  const [roundId, setRoundId] = useState(() => Number(localStorage.getItem(ROUND_KEY) ?? "0"));
  const [round, setRound] = useState<RoundView | null>(null);
  const [roster, setRoster] = useState<PlayerView[]>([]);
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [target, setTarget] = useState<string | null>(null);
  const [mode, setMode] = useState<"demoSlot" | "orao">("demoSlot");
  const [bonusSol, setBonusSol] = useState("0.05");
  const [reserveLamports, setReserveLamports] = useState<number | null>(null);

  const signer = useMemo<WalletSigner | null>(() => {
    if (practice) return keypairSigner(practice);
    if (wallet.publicKey && wallet.signTransaction) {
      return { publicKey: wallet.publicKey, signTransaction: wallet.signTransaction };
    }
    return null;
  }, [practice, wallet.publicKey, wallet.signTransaction]);

  const refresh = useCallback(async () => {
    try {
      setReserveLamports(await connection.getBalance(reservePda()));
    } catch {
      /* keep the last reserve figure if this poll is rate-limited */
    }
    const nextConfig = await loadConfig(connection);
    setConfig(nextConfig);
    if (!nextConfig || nextConfig.nextRoundId === 0) {
      setRound(null);
      setRoster([]);
      return;
    }
    const preferred = Number(localStorage.getItem(ROUND_KEY) ?? String(roundId));
    const id = Math.min(preferred, nextConfig.nextRoundId - 1);
    const nextRound = await loadRound(connection, id);
    setRound(nextRound);
    if (!nextRound) {
      setRoster([]);
      return;
    }
    const roundKey = new PublicKey(nextRound.address);
    const players = await Promise.all(
      nextRound.players.map((user) => loadPlayer(connection, roundKey, new PublicKey(user))),
    );
    setRoster(players.filter((player): player is PlayerView => player !== null));
  }, [connection, roundId]);

  useEffect(() => {
    refresh().catch(() => undefined);
  }, [refresh]);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      refresh().catch(() => undefined);
    }, 3000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  useEffect(() => {
    localStorage.setItem(ROUND_KEY, String(roundId));
  }, [roundId]);

  const me = roster.find((player) => player.user === signer?.publicKey.toBase58());
  const latestId = config ? Math.max(0, config.nextRoundId - 1) : 0;

  async function run(action: () => Promise<unknown>, success: string) {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      await action();
      setNote(success);
      await refresh();
    } catch (reason) {
      setError(messageOf(reason));
    } finally {
      setBusy(false);
    }
  }

  async function hostTurbo() {
    if (!signer || !config) return;
    const chainNow = Math.floor(Date.now() / 1000);
    const id = config.nextRoundId;
    await run(async () => {
      await createRound(connection, signer, id, {
        entry: Math.round(0.05 * LAMPORTS),
        sponsorBonus: Math.round(Number(bonusSol) * LAMPORTS),
        maxPlayers: 8,
        minPlayers: 2,
        windows: 2,
        windowDuration: 30,
        minGap: 4,
        maxGap: 4,
        stallTimeout: 180,
        joinUntil: chainNow + 180,
        mode,
      });
      localStorage.setItem(ROUND_KEY, String(id));
      setRoundId(id);
    }, `Round ${id} is open. Thirty-second windows and a four-second gap. Demo randomness (not VRF).`);
  }

  const phase = describe(round, me, now);
  const verifiedGap =
    round && round.lastRandomness.some((byte) => byte !== 0)
      ? gapFromRandomness(round.lastRandomness, round.minGap, round.maxGap)
      : null;

  return (
    <div className="app">
      <header className="top">
        <div className="mark">
          <div className="eyebrow">Devnet savings battle royale</div>
          <h1>
            Last Saver <em>Standing</em>
          </h1>
          <p className="lede">
            Lock the same deposit with everyone else. Random check-in windows open. Miss one and
            you leave the prize — your SOL still comes back.
          </p>
        </div>
        <div className="wallet-row">
          <WalletMultiButton />
          <button
            className="ghost"
            data-testid="practice-key"
            onClick={() => {
              const next = Keypair.generate();
              rememberPractice(next);
              setPractice(next);
            }}
            type="button"
          >
            {practice ? displayName(practice.publicKey.toBase58()) : "Practice key"}
          </button>
          {practice && (
            <button
              className="ghost"
              type="button"
              disabled={busy}
              onClick={() =>
                run(async () => {
                  const signature = await connection.requestAirdrop(practice.publicKey, 2 * LAMPORTS);
                  const latest = await connection.getLatestBlockhash();
                  await connection.confirmTransaction({ signature, ...latest }, "confirmed");
                }, "Airdrop confirmed. Devnet faucets are often rate-limited; a local validator confirms immediately.")
              }
            >
              Airdrop 2 SOL
            </button>
          )}
        </div>
      </header>

      <section className="banner">
        <div>
          <span className="tag">No-loss</span>
          <strong>Your deposit is not the prize.</strong>
          <p>
            Principal sits in the round vault until settlement, then anyone can push it back to
            your wallet. Eliminated players get the same deposit back as the winner.
          </p>
        </div>
        <div>
          <span className="tag">Mock yield / sponsor reserve</span>
          <strong>The prize is sponsor money, labelled as mock yield.</strong>
          <p>
            A published {config ? (config.annualYieldBps / 100).toFixed(2) : "5.00"}% APY is quoted
            on the pooled deposits and paid from a separate reserve. It is not interest from a
            lending protocol. On a two-minute round the quote is dust; the visible pot is the
            sponsor bonus.
          </p>
        </div>
      </section>

      {!config && signer && (
        <section className="card">
          <h2>Open the arena</h2>
          <p className="muted">One-time setup. The yield source is recorded on-chain as mock reserve.</p>
          <div className="actions">
            <button className="btn" disabled={busy} type="button" onClick={() => run(() => initialize(connection, signer), "Arena initialized at 5% simulated APY.")}>
              Initialize
            </button>
          </div>
        </section>
      )}

      {config && signer && (
        <section className="card" style={{ marginBottom: 18 }}>
          <h2>Host a demo round</h2>
          <p className="ok-note" data-testid="reserve-balance">
            Sponsor reserve {reserveLamports === null ? "…" : formatSol(reserveLamports)} SOL. Mock yield / sponsor reserve — not interest from a lending protocol.
          </p>
          <div className="form">
            <label>
              Randomness
              <select value={mode} onChange={(event) => setMode(event.target.value as "demoSlot" | "orao")}>
                <option value="demoSlot">Demo slot-hash commit-reveal (fast, not a VRF)</option>
                <option value="orao">ORAO VRF on devnet</option>
              </select>
            </label>
            <label>
              Sponsor bonus (SOL, from the reserve)
              <input value={bonusSol} onChange={(event) => setBonusSol(event.target.value)} />
            </label>
          </div>
          <div className="actions" style={{ marginTop: 10 }}>
            <button className="btn" data-testid="create-round" disabled={busy} type="button" onClick={hostTurbo}>
              Create demo round
            </button>
            <button
              className="ghost"
              data-testid="fund-reserve"
              disabled={busy}
              type="button"
              onClick={() =>
                run(
                  () => fundReserve(connection, signer, Math.round(Number(bonusSol) * LAMPORTS) || Math.round(0.2 * LAMPORTS)),
                  "Reserve funded. This SOL can become prize money. It is not a player deposit.",
                )
              }
            >
              Fund the reserve
            </button>
            <button className="ghost" type="button" onClick={() => setRoundId(latestId)}>
              Latest round ({latestId})
            </button>
          </div>
        </section>
      )}

      <div className="layout">
        <section className="card stage">
          <div className="row">
            <label>
              Round
              <input
                type="number"
                min={0}
                value={roundId}
                onChange={(event) => setRoundId(Number(event.target.value))}
              />
            </label>
          </div>
          {round ? (
            <>
              <div className="ring-wrap">
                <Ring phase={phase} />
                <div>
                  <h2>{phase.title}</h2>
                  <p className="muted">{phase.detail}</p>
                  {round.mode === "demoSlot" && (
                    <p className="ok-note" data-testid="randomness-label">
                      Demo randomness (not VRF)
                    </p>
                  )}
                  <p className="ok-note">{randomnessCopy(round, verifiedGap)}</p>
                  <p className="ok-note">
                    Principal locked in the vault: {formatSol(round.principal)} SOL. Prize SOL is separate.
                  </p>
                </div>
              </div>
              <div className="stats">
                <div className="stat">
                  <b>{round.aliveCount}</b>
                  <span>Still standing</span>
                </div>
                <div className="stat gold">
                  <b>{formatSol(round.prize)}</b>
                  <span>Prize pot SOL</span>
                </div>
                <div className="stat">
                  <b>{formatSol(round.entry)}</b>
                  <span>Deposit SOL</span>
                </div>
              </div>
              <div className="stats">
                <div className="stat">
                  <b>{formatSol(round.sponsorBonus)}</b>
                  <span>Sponsor bonus</span>
                </div>
                <div className="stat">
                  <b>{formatSol(round.yieldAccrued)}</b>
                  <span>Mock yield paid</span>
                </div>
                <div className="stat">
                  <b>
                    {round.standardsOpened}/{round.standardsScheduled}
                  </b>
                  <span>Standard windows</span>
                </div>
              </div>
              {signer && (
                <div className="actions">
                  {round.status === "forming" && !me && (
                    <button className="btn big" data-testid="join" disabled={busy} type="button" onClick={() => run(() => joinRound(connection, signer, round.id), "You're in. Deposit locked until the round ends.")}>
                      Join · {formatSol(round.entry)} SOL
                    </button>
                  )}
                  {round.status === "forming" && round.playerCount >= round.minPlayers && (
                    <button className="ghost" data-testid="start-round" disabled={busy} type="button" onClick={() => run(() => startRound(connection, signer, round.id), "Round started. Draw the first window.")}>
                      Start round
                    </button>
                  )}
                  {round.status === "needCommit" && (
                    <button
                      className="btn big"
                      data-testid="draw-window"
                      disabled={busy}
                      type="button"
                      onClick={() =>
                        run(
                          () =>
                            round.mode === "orao"
                              ? drawOraoWindow(connection, signer, round.id)
                              : drawDemoWindow(connection, signer, round.id),
                          "Window scheduled from the revealed randomness.",
                        )
                      }
                    >
                      {round.mode === "orao" ? "Request ORAO window" : "Draw next window"}
                    </button>
                  )}
                  {round.status === "committed" && round.mode === "demoSlot" && (
                    <button className="ghost" data-testid="reveal-window" disabled={busy} type="button" onClick={() => run(() => revealDemo(connection, signer, round.id), "Window revealed from the committed slot hash.")}>
                      Reveal when the slot hash lands
                    </button>
                  )}
                  {me && phase.canCheckIn && (
                    <button className="btn big pulse" data-testid="check-in" disabled={busy} type="button" onClick={() => run(() => checkIn(connection, signer, round.id), "Checked in. You keep your shield charge.")}>
                      Check in
                    </button>
                  )}
                  {me && phase.canShield && (
                    <button className="ghost" disabled={busy} type="button" onClick={() => run(() => armShield(connection, signer, round.id), "Shield armed for this window only.")}>
                      Arm shield ({me.shield})
                    </button>
                  )}
                  {me && phase.canJolt && target && (
                    <button className="danger" data-testid="jolt" disabled={busy} type="button" onClick={() => run(() => sabotage(connection, signer, round.id, new PublicKey(target)), "Jolt sent. They owe one extra check-in of the same length.")}>
                      Jolt {displayName(target)}
                    </button>
                  )}
                  {round.status === "open" && now >= round.closeAt && (
                    <button
                      className="btn"
                      data-testid="resolve"
                      disabled={busy}
                      type="button"
                      onClick={() =>
                        run(async () => {
                          for (const player of roster) {
                            if (!mustResolve(round, player, Math.floor(Date.now() / 1000))) continue;
                            try {
                              await resolveAbsence(connection, signer, round.id, new PublicKey(player.user));
                            } catch (error) {
                              const text = messageOf(error);
                              if (!/Already|Eliminated|NotRequired/.test(text)) throw error;
                            }
                          }
                          await finalizeWindow(connection, signer, round.id);
                        }, "Window resolved.")
                      }
                    >
                      Resolve window
                    </button>
                  )}
                  {(round.status === "settled" || round.status === "cancelled") && me && !me.withdrawn && (
                    <button className="btn big" data-testid="withdraw" disabled={busy} type="button" onClick={() => run(() => withdrawPrincipal(connection, signer, round.id, signer.publicKey), "Deposit returned in full.")}>
                      Withdraw deposit
                    </button>
                  )}
                  {round.status === "settled" && me?.alive && !me.claimed && round.prizePerWinner > 0 && (
                    <button className="ghost" data-testid="claim" disabled={busy} type="button" onClick={() => run(() => claimPrize(connection, signer, round.id, signer.publicKey), "Prize claimed from the sponsor reserve.")}>
                      Claim {formatSol(round.prizePerWinner)} SOL
                    </button>
                  )}
                  {["needCommit", "committed", "open"].includes(round.status) && now > round.progressDeadline && (
                    <button className="danger" disabled={busy} type="button" onClick={() => run(() => refundStalled(connection, signer, round.id), "Round cancelled. Deposits can be withdrawn. Prize went back to the reserve.")}>
                      Refund stalled round
                    </button>
                  )}
                </div>
              )}
            </>
          ) : (
            <p className="muted">No round at this id yet.</p>
          )}
          {note && (
            <p className="ok-note" data-testid="status-note">
              {note}
            </p>
          )}
          {error && (
            <div className="toast" data-testid="error-toast">
              {error}
            </div>
          )}
        </section>

        <aside>
          <section className="card">
            <h2>Who's standing</h2>
            <div className="list">
              {roster.map((player) => (
                <button
                  key={player.user}
                  type="button"
                  data-testid="player"
                  data-user={player.user}
                  className={`person ${player.alive ? "" : "out"} ${target === player.user ? "picked" : ""}`}
                  onClick={() => setTarget(player.user)}
                >
                  <div>
                    <div className="name">
                      {displayName(player.user)}
                      {player.user === signer?.publicKey.toBase58() ? " · you" : ""}
                    </div>
                    <div className="chips">
                      <span className={`chip ${player.alive ? "live" : "dead"}`}>
                        {player.alive ? "Standing" : "Out"}
                      </span>
                      <span className="chip">Shield {player.shield}</span>
                      <span className="chip">Jolt {player.sabotage}</span>
                      {round && player.shieldArmedFor === round.windowId && player.shieldArmedFor > 0 && (
                        <span className="chip warn">Shield armed</span>
                      )}
                      {player.joltPending && <span className="chip warn">Jolted</span>}
                      {player.withdrawn && <span className="chip">Deposit back</span>}
                    </div>
                  </div>
                </button>
              ))}
              {roster.length === 0 && <p className="muted">Nobody has joined.</p>}
            </div>
            <p className="muted" style={{ marginTop: 10 }}>
              Pick a rival, then jolt them during a standard window. One jolt each. One shield each.
              A jolt adds one extra check-in; it does not touch deposits.
            </p>
          </section>
        </aside>
      </div>

      <footer className="foot">
        <span>
          {cluster} · <code>{endpoint}</code>
        </span>
        <span>
          Program <code>{PROGRAM_ID.toBase58()}</code>
        </span>
      </footer>
    </div>
  );
}

function Ring({ phase }: { phase: PhaseCopy }) {
  const radius = 52;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference * (1 - phase.progress);
  return (
    <div className="ring" aria-hidden="true">
      <svg viewBox="0 0 120 120">
        <circle className="track" cx="60" cy="60" r={radius} />
        <circle className="bar" cx="60" cy="60" r={radius} strokeDasharray={circumference} strokeDashoffset={offset} />
      </svg>
      <div className="ring-copy">
        <div>
          <strong>{phase.clock}</strong>
          <span>{phase.clockLabel}</span>
        </div>
      </div>
    </div>
  );
}

type PhaseCopy = {
  title: string;
  detail: string;
  clock: string;
  clockLabel: string;
  progress: number;
  canCheckIn: boolean;
  canShield: boolean;
  canJolt: boolean;
};

function describe(round: RoundView | null, me: PlayerView | undefined, now: number): PhaseCopy {
  if (!round) {
    return { title: "No round", detail: "Host one to start.", clock: "—", clockLabel: "idle", progress: 0, canCheckIn: false, canShield: false, canJolt: false };
  }
  if (round.status === "forming") {
    return {
      title: "Doors open",
      detail: `${round.playerCount}/${round.maxPlayers} joined. Need ${round.minPlayers} to start.`,
      clock: String(round.playerCount),
      clockLabel: "joined",
      progress: round.playerCount / round.maxPlayers,
      canCheckIn: false,
      canShield: false,
      canJolt: false,
    };
  }
  if (round.status === "needCommit") {
    return {
      title: round.pendingKind === "jolt" ? "A jolt window is waiting" : "Seal the next window",
      detail: "The gap is chosen when randomness is revealed, not before.",
      clock: "?",
      clockLabel: "unsealed",
      progress: 0.08,
      canCheckIn: false,
      canShield: false,
      canJolt: false,
    };
  }
  if (round.status === "committed") {
    return {
      title: "Randomness is locked",
      detail: round.mode === "orao" ? "Waiting for the ORAO fulfillment." : "Waiting for the committed slot hash.",
      clock: "…",
      clockLabel: "pending",
      progress: 0.2,
      canCheckIn: false,
      canShield: false,
      canJolt: false,
    };
  }
  if (round.status === "open") {
    if (now < round.openAt) {
      const left = round.openAt - now;
      const span = Math.max(1, round.openAt - (round.openAt - round.lastGap));
      return {
        title: "Window is on the clock",
        detail: `It opens in ${left}s. Gap was ${round.lastGap}s.`,
        clock: formatClock(left),
        clockLabel: "until open",
        progress: 1 - left / span,
        canCheckIn: false,
        canShield: false,
        canJolt: false,
      };
    }
    const left = Math.max(0, round.closeAt - now);
    const span = Math.max(1, round.windowDuration);
    const attending = Boolean(me && me.alive && me.checkedWindow !== round.windowId && (round.currentKind === "standard" || me.joltPending));
    return {
      title: round.currentKind === "jolt" ? "Jolt check-in" : "Check-in is open",
      detail: attending ? "You need to tap in before the ring empties." : "You are not required for this window.",
      clock: formatClock(left),
      clockLabel: "left",
      progress: left / span,
      canCheckIn: attending && left > 0,
      canShield: attending && left > 0 && (me?.shield ?? 0) > 0,
      canJolt: Boolean(me && me.alive && me.sabotage > 0 && round.currentKind === "standard" && left > 0),
    };
  }
  if (round.status === "cancelled") {
    return {
      title: round.prizeRefunded ? "Stalled and refunded" : "Cancelled",
      detail: "Deposits can be withdrawn. Any prize was returned to the sponsor reserve.",
      clock: "0",
      clockLabel: "closed",
      progress: 1,
      canCheckIn: false,
      canShield: false,
      canJolt: false,
    };
  }
  return {
    title: round.winners === 0 ? "Nobody lasted" : `${round.winners} still standing`,
    detail:
      round.prizePerWinner > 0
        ? `Each survivor can claim ${formatSol(round.prizePerWinner)} SOL. Everyone can withdraw their deposit.`
        : "Everyone can withdraw their deposit.",
    clock: String(round.winners),
    clockLabel: "winners",
    progress: 1,
    canCheckIn: false,
    canShield: false,
    canJolt: false,
  };
}

function randomnessCopy(round: RoundView, verified: number | null) {
  if (round.mode === "orao") {
    return verified === null
      ? "This round uses ORAO VRF. The seed is committed while the request is still pending."
      : `ORAO gap ${round.lastGap}s matches the fulfilled value (${verified}s).`;
  }
  return verified === null
    ? "Demo mode uses Solana slot hashes, not a VRF. A leader can bias them. Don't treat the prize as production-fair."
    : `Demo gap ${round.lastGap}s matches the slot-hash mix (${verified}s). This is commit-reveal, not ORAO VRF.`;
}

function formatClock(seconds: number) {
  const safe = Math.max(0, seconds);
  return `${Math.floor(safe / 60)}:${String(safe % 60).padStart(2, "0")}`;
}

function messageOf(reason: unknown) {
  if (reason instanceof TxFailed) return reason.message;
  if (reason instanceof Error) return reason.message;
  return "Something went wrong";
}
