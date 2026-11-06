use anchor_lang::prelude::*;

pub const CONFIG_SEED: &[u8] = b"config";
pub const YIELD_SEED: &[u8] = b"yield";
pub const ROUND_SEED: &[u8] = b"round";
pub const PLAYER_SEED: &[u8] = b"player";
pub const MAX_PLAYERS: usize = 16;

/// Prize lamports are paid from the sponsor reserve, never from deposits.
pub const YIELD_SOURCE_MOCK_RESERVE: u8 = 1;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
#[repr(u8)]
pub enum RoundStatus {
    Forming,
    NeedCommit,
    Committed,
    Open,
    Settled,
    Cancelled,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
#[repr(u8)]
pub enum WindowKind {
    None,
    Standard,
    Jolt,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
#[repr(u8)]
pub enum RandomnessMode {
    /// Commit-reveal on the slot-hash sysvar. Fine for demos and tests.
    /// Not a VRF: a slot leader can bias hashes. The account stores this
    /// mode so clients can refuse to treat it as a production draw.
    DemoSlot,
    /// ORAO VRF (`VRFzZoJdhFWL8rkvu87LpKM3RbcVezpMEc6X5GVDr7y`).
    /// The seed is committed while the request is still pending.
    Orao,
}

#[account]
#[derive(InitSpace, Debug)]
pub struct Config {
    pub admin: Pubkey,
    /// Simulated APY in basis points. 500 = 5%. Snapshotted onto each round at start.
    pub annual_yield_bps: u16,
    /// Always [`YIELD_SOURCE_MOCK_RESERVE`] in this build.
    pub yield_source: u8,
    pub next_round_id: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace, Debug)]
pub struct YieldReserve {
    pub admin: Pubkey,
    pub bump: u8,
}

#[account]
#[derive(InitSpace, Debug)]
pub struct Round {
    pub id: u64,
    pub authority: Pubkey,
    pub entry_lamports: u64,
    pub sponsor_bonus: u64,
    pub max_players: u8,
    pub min_players: u8,
    pub player_count: u8,
    pub alive_count: u8,
    pub standards_scheduled: u8,
    pub standards_opened: u8,
    pub window_duration: i64,
    pub min_gap: i64,
    pub max_gap: i64,
    pub stall_timeout: i64,
    pub join_until: i64,
    pub started_at: i64,
    pub last_yield_ts: i64,
    pub settled_at: i64,
    pub progress_deadline: i64,
    pub principal_lamports: u64,
    pub prize_lamports: u64,
    pub prize_dust: u64,
    pub yield_accrued: u64,
    pub yield_bps: u16,
    pub yield_source: u8,
    pub winners: u8,
    pub prize_per_winner: u64,
    pub prize_refunded: bool,
    pub status: RoundStatus,
    pub mode: RandomnessMode,
    pub pending_kind: WindowKind,
    pub current_kind: WindowKind,
    pub window_id: u16,
    pub open_at: i64,
    pub close_at: i64,
    pub attendees_remaining: u8,
    pub jolt_marks: u8,
    pub commit_slot: u64,
    pub commit_ts: i64,
    pub last_gap: i64,
    pub commit_seed: [u8; 32],
    pub last_seed: [u8; 32],
    pub last_randomness: [u8; 64],
    pub bump: u8,
    pub players: [Pubkey; MAX_PLAYERS],
}

#[account]
#[derive(InitSpace, Debug)]
pub struct Player {
    pub round: Pubkey,
    pub user: Pubkey,
    pub alive: bool,
    pub shield_charges: u8,
    pub sabotage_charges: u8,
    pub shield_armed_for: u16,
    pub checked_window: u16,
    pub resolved_window: u16,
    pub jolt_pending: bool,
    pub jolts_received: u8,
    pub principal_withdrawn: bool,
    pub prize_claimed: bool,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CreateRoundArgs {
    pub entry_lamports: u64,
    pub sponsor_bonus: u64,
    pub max_players: u8,
    pub min_players: u8,
    pub windows: u8,
    pub window_duration: i64,
    pub min_gap: i64,
    pub max_gap: i64,
    pub stall_timeout: i64,
    pub join_until: i64,
    pub mode: RandomnessMode,
}

#[event]
pub struct Joined {
    pub round_id: u64,
    pub user: Pubkey,
    pub player_count: u8,
}

#[event]
pub struct WindowOpened {
    pub round_id: u64,
    pub window_id: u16,
    pub kind: u8,
    pub open_at: i64,
    pub close_at: i64,
    pub gap: i64,
}

#[event]
pub struct PlayerEliminated {
    pub round_id: u64,
    pub user: Pubkey,
    pub window_id: u16,
}

#[event]
pub struct ShieldConsumed {
    pub round_id: u64,
    pub user: Pubkey,
    pub window_id: u16,
}

#[event]
pub struct PlayerJolted {
    pub round_id: u64,
    pub attacker: Pubkey,
    pub target: Pubkey,
}

#[event]
pub struct RoundSettled {
    pub round_id: u64,
    pub winners: u8,
    pub prize_per_winner: u64,
    pub yield_accrued: u64,
    pub yield_source: u8,
    pub prize_refunded: bool,
}
