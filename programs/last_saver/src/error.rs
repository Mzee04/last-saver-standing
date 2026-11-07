use anchor_lang::prelude::*;

#[error_code]
pub enum LastSaverError {
    #[msg("Round configuration is invalid")]
    BadConfig,
    #[msg("Round id does not match the next round")]
    BadRoundId,
    #[msg("Round is not accepting deposits")]
    NotForming,
    #[msg("Join window has closed")]
    JoinClosed,
    #[msg("Round is full")]
    RoundFull,
    #[msg("Not enough players to start")]
    NotEnoughPlayers,
    #[msg("Only the round creator can start early")]
    EarlyStart,
    #[msg("Round is in the wrong phase")]
    BadPhase,
    #[msg("This round uses a different randomness mode")]
    WrongMode,
    #[msg("Seed must be non-zero")]
    BadSeed,
    #[msg("VRF account failed verification")]
    BadVrf,
    #[msg("VRF seed does not match the commit")]
    VrfSeedMismatch,
    #[msg("VRF result is not ready")]
    VrfPending,
    #[msg("VRF was already fulfilled before commit")]
    VrfAlreadyFulfilled,
    #[msg("Slot hash for the committed slot is not available yet")]
    SlotHashUnavailable,
    #[msg("Check-in window is not open")]
    WindowClosed,
    #[msg("Player is already eliminated")]
    Eliminated,
    #[msg("Already checked in for this window")]
    AlreadyCheckedIn,
    #[msg("Player was already resolved for this window")]
    AlreadyResolved,
    #[msg("Player does not have to attend this window")]
    NotRequired,
    #[msg("No shield charge left")]
    NoShield,
    #[msg("No sabotage charge left")]
    NoSabotage,
    #[msg("Cannot sabotage yourself")]
    SelfSabotage,
    #[msg("Target cannot be jolted")]
    CannotJolt,
    #[msg("Sabotage is only available during a standard window")]
    JoltClosed,
    #[msg("Every required player must be checked in or resolved first")]
    UnresolvedPlayers,
    #[msg("Round is still active")]
    RoundStillActive,
    #[msg("Principal already withdrawn")]
    AlreadyWithdrawn,
    #[msg("Prize already claimed")]
    AlreadyClaimed,
    #[msg("Round is not settled")]
    NotSettled,
    #[msg("No prize to claim")]
    NoPrize,
    #[msg("Vault accounting mismatch")]
    Accounting,
    #[msg("Vault cannot cover this transfer without touching locked funds")]
    InsufficientVault,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Round has not stalled")]
    NotStalled,
    #[msg("Round cannot be cancelled")]
    CannotCancel,
    #[msg("Yield rate must be between 1 and 10000 basis points")]
    BadYieldRate,
    #[msg("No dust to sweep")]
    NoDust,
}
