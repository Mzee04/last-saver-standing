//! End-to-end rules for Last Saver Standing.
//!
//! Runs the program in-process through solana-program-test (not a BPF deploy).
//! `anchor build` is what produces the devnet binary.

use anchor_lang::{AccountDeserialize, AnchorSerialize};
use last_saver::{
    demo_randomness, gap_seconds, CreateRoundArgs, Player, RandomnessMode, Round, RoundStatus,
    WindowKind, CONFIG_SEED, ORAO_REQUEST_SEED, ORAO_V2_DISC, ORAO_VRF_ID, PLAYER_SEED, ROUND_SEED,
    SECONDS_PER_YEAR, YIELD_SEED,
};
use solana_program_test::{processor, ProgramTest, ProgramTestContext};
use std::sync::atomic::{AtomicU64, Ordering};

use solana_sdk::{
    account::AccountSharedData,
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction, system_program,
    transaction::Transaction,
};

static TX_NONCE: AtomicU64 = AtomicU64::new(1);

/// Anchor's generated entry ties the account-slice lifetime to `AccountInfo`.
/// The program-test builtin wants those lifetimes independent. They are the
/// same allocation for this call.
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    last_saver::entry(
        program_id,
        unsafe { std::mem::transmute(accounts) },
        instruction_data,
    )
}

fn sighash(name: &str) -> [u8; 8] {
    let hashed = hash(format!("global:{name}").as_bytes()).to_bytes();
    let mut out = [0u8; 8];
    out.copy_from_slice(&hashed[..8]);
    out
}

fn ix(name: &str, accounts: Vec<AccountMeta>, args: &[u8]) -> Instruction {
    let mut data = sighash(name).to_vec();
    data.extend_from_slice(args);
    Instruction {
        program_id: last_saver::ID,
        accounts,
        data,
    }
}

fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &last_saver::ID).0
}

fn config_pda() -> Pubkey {
    pda(&[CONFIG_SEED])
}

fn reserve_pda() -> Pubkey {
    pda(&[YIELD_SEED])
}

fn round_pda(id: u64) -> Pubkey {
    pda(&[ROUND_SEED, &id.to_le_bytes()])
}

fn player_pda(round: &Pubkey, user: &Pubkey) -> Pubkey {
    pda(&[PLAYER_SEED, round.as_ref(), user.as_ref()])
}

fn meta(key: Pubkey, writable: bool, signer: bool) -> AccountMeta {
    AccountMeta {
        pubkey: key,
        is_signer: signer,
        is_writable: writable,
    }
}

async fn start() -> ProgramTestContext {
    let mut test = ProgramTest::new(
        "last_saver",
        last_saver::ID,
        processor!(process_instruction),
    );
    test.prefer_bpf(false);
    test.start_with_context().await
}

async fn send(
    ctx: &mut ProgramTestContext,
    payer: &Keypair,
    instruction: Instruction,
    extra: &[&Keypair],
) -> std::result::Result<(), solana_program_test::BanksClientError> {
    let blockhash = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let mut signers: Vec<&Keypair> = vec![payer];
    for signer in extra {
        if signer.pubkey() != payer.pubkey() {
            signers.push(signer);
        }
    }
    // Identical retries (same blockhash, same instruction) are dropped as
    // duplicates. A unique compute-budget price keeps each attempt distinct.
    let nonce = TX_NONCE.fetch_add(1, Ordering::Relaxed);
    let mut price = vec![3u8];
    price.extend_from_slice(&nonce.to_le_bytes());
    let budget = Instruction {
        program_id: Pubkey::from_str_const("ComputeBudget111111111111111111111111111111"),
        accounts: vec![],
        data: price,
    };
    let tx = Transaction::new_signed_with_payer(
        &[budget, instruction],
        Some(&payer.pubkey()),
        &signers,
        blockhash,
    );
    ctx.banks_client.process_transaction(tx).await
}

fn expect_err(
    result: std::result::Result<(), solana_program_test::BanksClientError>,
    name: &str,
    index: u32,
) {
    let err = result.expect_err("expected an error");
    let text = format!("{err:?}");
    let custom = format!("Custom({})", 6000 + index);
    assert!(
        text.contains(name) || text.contains(&custom),
        "expected {name} or {custom} in {text}"
    );
}

async fn clock(ctx: &mut ProgramTestContext) -> Clock {
    ctx.banks_client.get_sysvar().await.unwrap()
}

async fn set_clock(ctx: &mut ProgramTestContext, slot: u64, unix_timestamp: i64) {
    let mut next = clock(ctx).await;
    next.slot = slot;
    next.unix_timestamp = unix_timestamp;
    ctx.set_sysvar(&next);
}

async fn lamports(ctx: &mut ProgramTestContext, key: &Pubkey) -> u64 {
    ctx.banks_client
        .get_account(*key)
        .await
        .unwrap()
        .map(|account| account.lamports)
        .unwrap_or(0)
}

async fn load_round(ctx: &mut ProgramTestContext, id: u64) -> Round {
    let account = ctx
        .banks_client
        .get_account(round_pda(id))
        .await
        .unwrap()
        .expect("round");
    Round::try_deserialize(&mut account.data.as_slice()).unwrap()
}

async fn load_player(ctx: &mut ProgramTestContext, round: &Pubkey, user: &Pubkey) -> Player {
    let account = ctx
        .banks_client
        .get_account(player_pda(round, user))
        .await
        .unwrap()
        .expect("player");
    Player::try_deserialize(&mut account.data.as_slice()).unwrap()
}

async fn fund(ctx: &mut ProgramTestContext, to: &Pubkey, amount: u64) {
    let ix = system_instruction::transfer(&ctx.payer.pubkey(), to, amount);
    // payer is owned by the context; clone the signature material via in-place use.
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    send(ctx, &payer, ix, &[]).await.unwrap();
}

fn args_at(now: i64, mode: RandomnessMode) -> CreateRoundArgs {
    CreateRoundArgs {
        entry_lamports: 1_000_000_000,
        sponsor_bonus: 500_000_000,
        max_players: 8,
        min_players: 3,
        windows: 2,
        window_duration: 30,
        min_gap: 0,
        max_gap: 0,
        stall_timeout: 300,
        join_until: now + 100_000,
        mode,
    }
}

async fn init_world(ctx: &mut ProgramTestContext) {
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let instruction = ix(
        "initialize",
        vec![
            meta(payer.pubkey(), true, true),
            meta(config_pda(), true, false),
            meta(reserve_pda(), true, false),
            meta(system_program::ID, false, false),
        ],
        &500u16.to_le_bytes(),
    );
    send(ctx, &payer, instruction, &[]).await.unwrap();
    let fund_ix = ix(
        "fund_reserve",
        vec![
            meta(payer.pubkey(), true, true),
            meta(reserve_pda(), true, false),
            meta(system_program::ID, false, false),
        ],
        &10_000_000_000u64.to_le_bytes(),
    );
    send(ctx, &payer, fund_ix, &[]).await.unwrap();
}

async fn create(ctx: &mut ProgramTestContext, id: u64, args: &CreateRoundArgs) {
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let mut data = id.to_le_bytes().to_vec();
    data.extend(args.try_to_vec().unwrap());
    let instruction = ix(
        "create_round",
        vec![
            meta(payer.pubkey(), true, true),
            meta(config_pda(), true, false),
            meta(round_pda(id), true, false),
            meta(system_program::ID, false, false),
        ],
        &data,
    );
    send(ctx, &payer, instruction, &[]).await.unwrap();
}

async fn join(ctx: &mut ProgramTestContext, round_id: u64, user: &Keypair) {
    let round = round_pda(round_id);
    let instruction = ix(
        "join",
        vec![
            meta(user.pubkey(), true, true),
            meta(round, true, false),
            meta(player_pda(&round, &user.pubkey()), true, false),
            meta(system_program::ID, false, false),
        ],
        &[],
    );
    send(ctx, user, instruction, &[]).await.unwrap();
}

async fn start_round(ctx: &mut ProgramTestContext, round_id: u64) {
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let instruction = ix(
        "start_round",
        vec![
            meta(payer.pubkey(), true, true),
            meta(config_pda(), false, false),
            meta(round_pda(round_id), true, false),
            meta(reserve_pda(), true, false),
        ],
        &[],
    );
    send(ctx, &payer, instruction, &[]).await.unwrap();
}

async fn try_start(
    ctx: &mut ProgramTestContext,
    round_id: u64,
) -> std::result::Result<(), solana_program_test::BanksClientError> {
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let instruction = ix(
        "start_round",
        vec![
            meta(payer.pubkey(), true, true),
            meta(config_pda(), false, false),
            meta(round_pda(round_id), true, false),
            meta(reserve_pda(), true, false),
        ],
        &[],
    );
    send(ctx, &payer, instruction, &[]).await
}

async fn commit_demo(ctx: &mut ProgramTestContext, round_id: u64, seed: [u8; 32]) {
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let instruction = ix(
        "commit_window",
        vec![
            meta(payer.pubkey(), true, true),
            meta(round_pda(round_id), true, false),
        ],
        &seed,
    );
    send(ctx, &payer, instruction, &[]).await.unwrap();
}

async fn reveal_demo(ctx: &mut ProgramTestContext, round_id: u64) {
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let slot_hashes = solana_sdk::sysvar::slot_hashes::id();
    let instruction = ix(
        "reveal_window_demo",
        vec![
            meta(payer.pubkey(), true, true),
            meta(round_pda(round_id), true, false),
            meta(slot_hashes, false, false),
        ],
        &[],
    );
    send(ctx, &payer, instruction, &[]).await.unwrap();
}

async fn open_demo_window(ctx: &mut ProgramTestContext, round_id: u64, seed: [u8; 32], when: i64) {
    commit_demo(ctx, round_id, seed).await;
    let committed = load_round(ctx, round_id).await;
    let next_slot = committed.commit_slot + 1;
    ctx.warp_to_slot(next_slot).unwrap();
    set_clock(ctx, next_slot, when).await;
    let slot_hashes = ctx
        .banks_client
        .get_account(solana_sdk::sysvar::slot_hashes::id())
        .await
        .unwrap()
        .expect("slot hashes");
    let slot_hash = last_saver::read_slot_hash(&slot_hashes.data, committed.commit_slot)
        .unwrap_or_else(|| {
            panic!(
                "missing hash for slot {} (sysvar len {}, head {:02x?})",
                committed.commit_slot,
                slot_hashes.data.len(),
                &slot_hashes.data[..slot_hashes.data.len().min(24)]
            );
        });
    let expected = demo_randomness(&slot_hash, &seed, committed.window_id + 1);
    let expected_gap = gap_seconds(&expected, committed.min_gap, committed.max_gap);
    reveal_demo(ctx, round_id).await;
    let opened = load_round(ctx, round_id).await;
    assert_eq!(opened.status, RoundStatus::Open);
    assert_eq!(opened.last_randomness, expected);
    assert_eq!(opened.last_gap, expected_gap);
    assert_eq!(opened.open_at, when + expected_gap);
    assert_eq!(opened.close_at, opened.open_at + opened.window_duration);
}

async fn check_in(ctx: &mut ProgramTestContext, round_id: u64, user: &Keypair) {
    let round = round_pda(round_id);
    let instruction = ix(
        "check_in",
        vec![
            meta(user.pubkey(), true, true),
            meta(round, true, false),
            meta(player_pda(&round, &user.pubkey()), true, false),
        ],
        &[],
    );
    send(ctx, user, instruction, &[]).await.unwrap();
}

async fn arm_shield(ctx: &mut ProgramTestContext, round_id: u64, user: &Keypair) {
    let round = round_pda(round_id);
    let instruction = ix(
        "arm_shield",
        vec![
            meta(user.pubkey(), true, true),
            meta(round, true, false),
            meta(player_pda(&round, &user.pubkey()), true, false),
        ],
        &[],
    );
    send(ctx, user, instruction, &[]).await.unwrap();
}

async fn sabotage(
    ctx: &mut ProgramTestContext,
    round_id: u64,
    attacker: &Keypair,
    target: &Pubkey,
) {
    let round = round_pda(round_id);
    let instruction = ix(
        "sabotage",
        vec![
            meta(attacker.pubkey(), true, true),
            meta(*target, false, false),
            meta(round, true, false),
            meta(player_pda(&round, &attacker.pubkey()), true, false),
            meta(player_pda(&round, target), true, false),
        ],
        &[],
    );
    send(ctx, attacker, instruction, &[]).await.unwrap();
}

async fn try_sabotage(
    ctx: &mut ProgramTestContext,
    round_id: u64,
    attacker: &Keypair,
    target: &Pubkey,
) -> std::result::Result<(), solana_program_test::BanksClientError> {
    let round = round_pda(round_id);
    let instruction = ix(
        "sabotage",
        vec![
            meta(attacker.pubkey(), true, true),
            meta(*target, false, false),
            meta(round, true, false),
            meta(player_pda(&round, &attacker.pubkey()), true, false),
            meta(player_pda(&round, target), true, false),
        ],
        &[],
    );
    send(ctx, attacker, instruction, &[]).await
}

async fn resolve(ctx: &mut ProgramTestContext, round_id: u64, user: &Pubkey) {
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let round = round_pda(round_id);
    let instruction = ix(
        "resolve_absence",
        vec![
            meta(payer.pubkey(), true, true),
            meta(*user, false, false),
            meta(round, true, false),
            meta(player_pda(&round, user), true, false),
        ],
        &[],
    );
    send(ctx, &payer, instruction, &[]).await.unwrap();
}

async fn finalize(ctx: &mut ProgramTestContext, round_id: u64) {
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let instruction = ix(
        "finalize_window",
        vec![
            meta(payer.pubkey(), true, true),
            meta(round_pda(round_id), true, false),
            meta(reserve_pda(), true, false),
        ],
        &[],
    );
    send(ctx, &payer, instruction, &[]).await.unwrap();
}

async fn withdraw(ctx: &mut ProgramTestContext, round_id: u64, user: &Pubkey) -> u64 {
    let before = lamports(ctx, user).await;
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let round = round_pda(round_id);
    let instruction = ix(
        "withdraw_principal",
        vec![
            meta(payer.pubkey(), true, true),
            meta(*user, true, false),
            meta(round, true, false),
            meta(player_pda(&round, user), true, false),
        ],
        &[],
    );
    send(ctx, &payer, instruction, &[]).await.unwrap();
    lamports(ctx, user).await - before
}

async fn claim(ctx: &mut ProgramTestContext, round_id: u64, user: &Pubkey) -> u64 {
    let before = lamports(ctx, user).await;
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let round = round_pda(round_id);
    let instruction = ix(
        "claim_prize",
        vec![
            meta(payer.pubkey(), true, true),
            meta(*user, true, false),
            meta(round, true, false),
            meta(player_pda(&round, user), true, false),
        ],
        &[],
    );
    send(ctx, &payer, instruction, &[]).await.unwrap();
    lamports(ctx, user).await - before
}

async fn try_claim(
    ctx: &mut ProgramTestContext,
    round_id: u64,
    user: &Pubkey,
) -> std::result::Result<(), solana_program_test::BanksClientError> {
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let round = round_pda(round_id);
    let instruction = ix(
        "claim_prize",
        vec![
            meta(payer.pubkey(), true, true),
            meta(*user, true, false),
            meta(round, true, false),
            meta(player_pda(&round, user), true, false),
        ],
        &[],
    );
    send(ctx, &payer, instruction, &[]).await
}

async fn try_withdraw(
    ctx: &mut ProgramTestContext,
    round_id: u64,
    user: &Pubkey,
) -> std::result::Result<(), solana_program_test::BanksClientError> {
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let round = round_pda(round_id);
    let instruction = ix(
        "withdraw_principal",
        vec![
            meta(payer.pubkey(), true, true),
            meta(*user, true, false),
            meta(round, true, false),
            meta(player_pda(&round, user), true, false),
        ],
        &[],
    );
    send(ctx, &payer, instruction, &[]).await
}

#[tokio::test]
async fn full_round_keeps_every_deposit_and_pays_prize_from_reserve() {
    let mut ctx = start().await;
    init_world(&mut ctx).await;
    let now = clock(&mut ctx).await.unix_timestamp;
    let mut round_args = args_at(now, RandomnessMode::DemoSlot);
    round_args.min_gap = 5;
    round_args.max_gap = 15;
    create(&mut ctx, 0, &round_args).await;

    let players: Vec<Keypair> = (0..3).map(|_| Keypair::new()).collect();
    for player in &players {
        fund(&mut ctx, &player.pubkey(), 5_000_000_000).await;
    }
    join(&mut ctx, 0, &players[0]).await;
    join(&mut ctx, 0, &players[1]).await;
    expect_err(try_start(&mut ctx, 0).await, "NotEnoughPlayers", 5);
    join(&mut ctx, 0, &players[2]).await;
    let again = join_result(&mut ctx, 0, &players[0]).await;
    assert!(again.is_err(), "duplicate join should fail");

    let reserve_before = lamports(&mut ctx, &reserve_pda()).await;
    start_round(&mut ctx, 0).await;
    let started = load_round(&mut ctx, 0).await;
    assert_eq!(started.status, RoundStatus::NeedCommit);
    assert_eq!(started.principal_lamports, 3_000_000_000);
    assert_eq!(started.prize_lamports, 500_000_000);
    assert_eq!(started.yield_source, 1);
    assert_eq!(
        lamports(&mut ctx, &reserve_pda()).await,
        reserve_before - 500_000_000
    );

    // Window 1. Gap comes from the slot hash; the assertion lives in open_demo_window.
    open_demo_window(&mut ctx, 0, [1u8; 32], started.started_at).await;
    let opened = load_round(&mut ctx, 0).await;
    assert_eq!(opened.current_kind, WindowKind::Standard);
    assert!((5..=15).contains(&opened.last_gap));
    set_clock(&mut ctx, opened.commit_slot + 1, opened.open_at).await;

    expect_err(
        try_withdraw(&mut ctx, 0, &players[0].pubkey()).await,
        "RoundStillActive",
        26,
    );

    arm_shield(&mut ctx, 0, &players[0]).await;
    check_in(&mut ctx, 0, &players[0]).await;
    arm_shield(&mut ctx, 0, &players[1]).await;
    // players[2] misses with no shield.

    set_clock(&mut ctx, opened.commit_slot + 1, opened.close_at).await;
    resolve(&mut ctx, 0, &players[1].pubkey()).await;
    resolve(&mut ctx, 0, &players[2].pubkey()).await;
    let shielded = load_player(&mut ctx, &round_pda(0), &players[1].pubkey()).await;
    let eliminated = load_player(&mut ctx, &round_pda(0), &players[2].pubkey()).await;
    let checked = load_player(&mut ctx, &round_pda(0), &players[0].pubkey()).await;
    assert!(shielded.alive);
    assert_eq!(shielded.shield_charges, 0);
    assert!(!eliminated.alive);
    assert!(checked.alive);
    assert_eq!(checked.shield_charges, 1, "checking in keeps the shield");

    expect_err(
        try_sabotage(&mut ctx, 0, &players[0], &players[0].pubkey()).await,
        "WindowClosed",
        15,
    );

    finalize(&mut ctx, 0).await;
    assert_eq!(
        load_round(&mut ctx, 0).await.status,
        RoundStatus::NeedCommit
    );

    // Window 2, then a jolt against the shielded player.
    let mid = load_round(&mut ctx, 0).await;
    open_demo_window(&mut ctx, 0, [2u8; 32], mid.started_at + 31).await;
    let second = load_round(&mut ctx, 0).await;
    set_clock(&mut ctx, second.commit_slot + 1, second.open_at).await;
    expect_err(
        try_sabotage(&mut ctx, 0, &players[0], &players[2].pubkey()).await,
        "CannotJolt",
        23,
    );
    sabotage(&mut ctx, 0, &players[0], &players[1].pubkey()).await;
    expect_err(
        try_sabotage(&mut ctx, 0, &players[0], &players[1].pubkey()).await,
        "NoSabotage",
        21,
    );
    check_in(&mut ctx, 0, &players[0]).await;
    check_in(&mut ctx, 0, &players[1]).await;
    set_clock(&mut ctx, second.commit_slot + 1, second.close_at).await;
    finalize(&mut ctx, 0).await;
    let after_standard = load_round(&mut ctx, 0).await;
    assert_eq!(after_standard.status, RoundStatus::NeedCommit);
    assert_eq!(after_standard.pending_kind, WindowKind::Jolt);

    open_demo_window(&mut ctx, 0, [3u8; 32], second.close_at).await;
    let jolt = load_round(&mut ctx, 0).await;
    assert_eq!(jolt.current_kind, WindowKind::Jolt);
    assert_eq!(jolt.attendees_remaining, 1);
    set_clock(&mut ctx, jolt.commit_slot + 1, jolt.close_at).await;
    resolve(&mut ctx, 0, &players[1].pubkey()).await;
    let jolted = load_player(&mut ctx, &round_pda(0), &players[1].pubkey()).await;
    assert!(
        !jolted.alive,
        "a missed jolt with no shield left eliminates"
    );

    // Jump one simulated year so the mock yield is an exact 5% of principal.
    let settle_at = started.started_at + SECONDS_PER_YEAR as i64;
    set_clock(&mut ctx, jolt.commit_slot + 1, settle_at).await;
    finalize(&mut ctx, 0).await;
    let settled = load_round(&mut ctx, 0).await;
    assert_eq!(settled.status, RoundStatus::Settled);
    assert_eq!(settled.winners, 1);
    assert_eq!(settled.yield_accrued, 150_000_000);
    assert_eq!(settled.prize_per_winner, 650_000_000);
    assert!(!settled.prize_refunded);
    assert_eq!(settled.principal_lamports, 3_000_000_000);
    assert_eq!(
        lamports(&mut ctx, &reserve_pda()).await,
        reserve_before - 500_000_000 - 150_000_000
    );

    assert_eq!(claim(&mut ctx, 0, &players[0].pubkey()).await, 650_000_000);
    expect_err(
        try_claim(&mut ctx, 0, &players[1].pubkey()).await,
        "Eliminated",
        16,
    );
    expect_err(
        try_claim(&mut ctx, 0, &players[2].pubkey()).await,
        "Eliminated",
        16,
    );

    for player in &players {
        assert_eq!(
            withdraw(&mut ctx, 0, &player.pubkey()).await,
            1_000_000_000,
            "principal comes back in full"
        );
    }
    expect_err(
        try_withdraw(&mut ctx, 0, &players[0].pubkey()).await,
        "AlreadyWithdrawn",
        27,
    );
    let empty = load_round(&mut ctx, 0).await;
    assert_eq!(empty.principal_lamports, 0);
    assert_eq!(empty.prize_lamports, 0);
}

async fn join_result(
    ctx: &mut ProgramTestContext,
    round_id: u64,
    user: &Keypair,
) -> std::result::Result<(), solana_program_test::BanksClientError> {
    let round = round_pda(round_id);
    let instruction = ix(
        "join",
        vec![
            meta(user.pubkey(), true, true),
            meta(round, true, false),
            meta(player_pda(&round, &user.pubkey()), true, false),
            meta(system_program::ID, false, false),
        ],
        &[],
    );
    send(ctx, user, instruction, &[]).await
}

#[tokio::test]
async fn orao_commit_rejects_a_known_result_and_reveal_uses_it() {
    let mut ctx = start().await;
    init_world(&mut ctx).await;
    let now = clock(&mut ctx).await.unix_timestamp;
    let mut round_args = args_at(now, RandomnessMode::Orao);
    round_args.min_players = 2;
    round_args.windows = 1;
    round_args.min_gap = 10;
    round_args.max_gap = 20;
    create(&mut ctx, 0, &round_args).await;
    let players: Vec<Keypair> = (0..2).map(|_| Keypair::new()).collect();
    for player in &players {
        fund(&mut ctx, &player.pubkey(), 3_000_000_000).await;
        join(&mut ctx, 0, player).await;
    }
    start_round(&mut ctx, 0).await;

    let seed = [9u8; 32];
    let (request, _) =
        Pubkey::find_program_address(&[ORAO_REQUEST_SEED, seed.as_ref()], &ORAO_VRF_ID);

    let mut randomness = [0u8; 64];
    randomness[..8].copy_from_slice(&15u64.to_le_bytes());
    set_orao(
        &mut ctx,
        &request,
        &fulfilled_bytes(&players[0].pubkey(), &seed, &randomness),
    );
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let early = ix(
        "commit_window_orao",
        vec![
            meta(payer.pubkey(), true, true),
            meta(round_pda(0), true, false),
            meta(request, false, false),
        ],
        &seed,
    );
    expect_err(
        send(&mut ctx, &payer, early, &[]).await,
        "VrfAlreadyFulfilled",
        13,
    );

    let next_slot = clock(&mut ctx).await.slot + 1;
    ctx.warp_to_slot(next_slot).unwrap();
    set_orao(
        &mut ctx,
        &request,
        &pending_bytes(&players[0].pubkey(), &seed),
    );
    let commit = ix(
        "commit_window_orao",
        vec![
            meta(payer.pubkey(), true, true),
            meta(round_pda(0), true, false),
            meta(request, false, false),
        ],
        &seed,
    );
    send(&mut ctx, &payer, commit, &[]).await.unwrap();

    let wrong = [4u8; 32];
    let (wrong_pda, _) =
        Pubkey::find_program_address(&[ORAO_REQUEST_SEED, wrong.as_ref()], &ORAO_VRF_ID);
    set_orao(
        &mut ctx,
        &wrong_pda,
        &fulfilled_bytes(&players[0].pubkey(), &wrong, &randomness),
    );
    let bad = ix(
        "reveal_window_orao",
        vec![
            meta(payer.pubkey(), true, true),
            meta(round_pda(0), true, false),
            meta(wrong_pda, false, false),
        ],
        &[],
    );
    expect_err(send(&mut ctx, &payer, bad, &[]).await, "BadVrf", 10);

    let reveal_slot = clock(&mut ctx).await.slot + 1;
    ctx.warp_to_slot(reveal_slot).unwrap();
    set_orao(
        &mut ctx,
        &request,
        &fulfilled_bytes(&players[0].pubkey(), &seed, &randomness),
    );
    let when = clock(&mut ctx).await.unix_timestamp;
    let reveal = ix(
        "reveal_window_orao",
        vec![
            meta(payer.pubkey(), true, true),
            meta(round_pda(0), true, false),
            meta(request, false, false),
        ],
        &[],
    );
    send(&mut ctx, &payer, reveal, &[]).await.unwrap();
    let opened = load_round(&mut ctx, 0).await;
    assert_eq!(opened.mode, RandomnessMode::Orao);
    assert_eq!(opened.last_randomness, randomness);
    assert_eq!(opened.last_gap, gap_seconds(&randomness, 10, 20));
    assert_eq!(opened.open_at, when + opened.last_gap);
    assert_eq!(opened.last_gap, 14);
}

#[tokio::test]
async fn cancel_and_stall_refund_return_principal() {
    let mut ctx = start().await;
    init_world(&mut ctx).await;
    let now = clock(&mut ctx).await.unix_timestamp;

    let mut forming = args_at(now, RandomnessMode::DemoSlot);
    forming.min_players = 2;
    forming.sponsor_bonus = 0;
    create(&mut ctx, 0, &forming).await;
    let user = Keypair::new();
    fund(&mut ctx, &user.pubkey(), 3_000_000_000).await;
    join(&mut ctx, 0, &user).await;
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let cancel = ix(
        "cancel_round",
        vec![
            meta(payer.pubkey(), true, true),
            meta(round_pda(0), true, false),
        ],
        &[],
    );
    send(&mut ctx, &payer, cancel, &[]).await.unwrap();
    assert_eq!(withdraw(&mut ctx, 0, &user.pubkey()).await, 1_000_000_000);

    let mut live = args_at(now, RandomnessMode::DemoSlot);
    live.min_players = 2;
    live.windows = 1;
    live.sponsor_bonus = 250_000_000;
    create(&mut ctx, 1, &live).await;
    let pair: Vec<Keypair> = (0..2).map(|_| Keypair::new()).collect();
    for player in &pair {
        fund(&mut ctx, &player.pubkey(), 3_000_000_000).await;
        join(&mut ctx, 1, player).await;
    }
    let reserve_before = lamports(&mut ctx, &reserve_pda()).await;
    start_round(&mut ctx, 1).await;
    commit_demo(&mut ctx, 1, [8u8; 32]).await;
    let committed = load_round(&mut ctx, 1).await;
    set_clock(
        &mut ctx,
        committed.commit_slot,
        committed.progress_deadline + 1,
    )
    .await;
    let refund = ix(
        "refund_stalled",
        vec![
            meta(payer.pubkey(), true, true),
            meta(round_pda(1), true, false),
            meta(reserve_pda(), true, false),
        ],
        &[],
    );
    send(&mut ctx, &payer, refund, &[]).await.unwrap();
    let cancelled = load_round(&mut ctx, 1).await;
    assert_eq!(cancelled.status, RoundStatus::Cancelled);
    assert!(cancelled.prize_refunded);
    assert_eq!(lamports(&mut ctx, &reserve_pda()).await, reserve_before);
    for player in &pair {
        assert_eq!(withdraw(&mut ctx, 1, &player.pubkey()).await, 1_000_000_000);
    }
}

#[tokio::test]
async fn empty_reserve_accrues_no_yield_and_does_not_touch_principal() {
    let mut ctx = start().await;
    let payer = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
    let instruction = ix(
        "initialize",
        vec![
            meta(payer.pubkey(), true, true),
            meta(config_pda(), true, false),
            meta(reserve_pda(), true, false),
            meta(system_program::ID, false, false),
        ],
        &500u16.to_le_bytes(),
    );
    send(&mut ctx, &payer, instruction, &[]).await.unwrap();
    let bonus = 400_000_000u64;
    let fund_ix = ix(
        "fund_reserve",
        vec![
            meta(payer.pubkey(), true, true),
            meta(reserve_pda(), true, false),
            meta(system_program::ID, false, false),
        ],
        &bonus.to_le_bytes(),
    );
    send(&mut ctx, &payer, fund_ix, &[]).await.unwrap();

    let now = clock(&mut ctx).await.unix_timestamp;
    let mut round_args = args_at(now, RandomnessMode::DemoSlot);
    round_args.min_players = 2;
    round_args.windows = 1;
    round_args.sponsor_bonus = bonus;
    round_args.min_gap = 0;
    round_args.max_gap = 0;
    create(&mut ctx, 0, &round_args).await;
    let players: Vec<Keypair> = (0..2).map(|_| Keypair::new()).collect();
    for player in &players {
        fund(&mut ctx, &player.pubkey(), 3_000_000_000).await;
        join(&mut ctx, 0, player).await;
    }
    start_round(&mut ctx, 0).await;
    let started = load_round(&mut ctx, 0).await;
    open_demo_window(&mut ctx, 0, [5u8; 32], started.started_at).await;
    let opened = load_round(&mut ctx, 0).await;
    set_clock(&mut ctx, opened.commit_slot + 1, opened.open_at).await;
    check_in(&mut ctx, 0, &players[0]).await;
    check_in(&mut ctx, 0, &players[1]).await;
    let settle_at = started.started_at + SECONDS_PER_YEAR as i64;
    set_clock(&mut ctx, opened.commit_slot + 1, settle_at).await;
    finalize(&mut ctx, 0).await;
    let settled = load_round(&mut ctx, 0).await;
    assert_eq!(settled.yield_accrued, 0, "reserve had nothing left to pay");
    assert_eq!(settled.prize_per_winner, bonus / 2);
    assert_eq!(settled.principal_lamports, 2_000_000_000);
    for player in &players {
        assert_eq!(withdraw(&mut ctx, 0, &player.pubkey()).await, 1_000_000_000);
    }
}

fn set_orao(ctx: &mut ProgramTestContext, address: &Pubkey, data: &[u8]) {
    let mut account = AccountSharedData::new(1_000_000_000, data.len(), &ORAO_VRF_ID);
    account.set_data_from_slice(data);
    ctx.set_account(address, &account);
}

fn pending_bytes(client: &Pubkey, seed: &[u8; 32]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&ORAO_V2_DISC);
    data.push(0);
    data.extend_from_slice(client.as_ref());
    data.extend_from_slice(seed);
    data.extend_from_slice(&0u32.to_le_bytes());
    data
}

fn fulfilled_bytes(client: &Pubkey, seed: &[u8; 32], randomness: &[u8; 64]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&ORAO_V2_DISC);
    data.push(1);
    data.extend_from_slice(client.as_ref());
    data.extend_from_slice(seed);
    data.extend_from_slice(randomness);
    data
}
