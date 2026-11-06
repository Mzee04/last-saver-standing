#![allow(unexpected_cfgs)]

//! Last Saver Standing
//!
//! Players deposit a fixed amount of SOL into a round vault. Random check-in
//! windows open. Miss one and you are eliminated, but the deposit is still
//! yours when the round ends (or if it is cancelled). The prize pot is a
//! sponsor bonus plus simulated yield, both paid from a separate reserve.
//! Principal is never the funding source for the prize.
//!
//! Randomness:
//! - `Orao`: the crank commits a seed while the ORAO VRF request is still
//!   pending, then reveals the fulfilled 64 bytes. Program id
//!   `VRFzZoJdhFWL8rkvu87LpKM3RbcVezpMEc6X5GVDr7y` (devnet and mainnet).
//! - `DemoSlot`: commit-reveal against the slot-hash sysvar so a two-minute
//!   demo does not wait on the oracle. This is not a VRF.

use anchor_lang::prelude::*;

mod error;
mod randomness;
mod state;
mod yield_math;

pub use error::LastSaverError;
pub use randomness::{
    demo_randomness, gap_seconds, parse_orao, read_slot_hash, ORAO_REQUEST_SEED, ORAO_V2_DISC,
    ORAO_VRF_ID,
};
pub use state::*;
pub use yield_math::{simulated_yield, SECONDS_PER_YEAR};

declare_id!("52MzPzQgqVGXYZNRAbF1nivLSUh7NQgYAbhEoFE7NUyb");

#[program]
pub mod last_saver {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, annual_yield_bps: u16) -> Result<()> {
        handlers::initialize(ctx, annual_yield_bps)
    }

    pub fn fund_reserve(ctx: Context<FundReserve>, amount: u64) -> Result<()> {
        handlers::fund_reserve(ctx, amount)
    }

    pub fn set_yield_bps(ctx: Context<SetYieldBps>, annual_yield_bps: u16) -> Result<()> {
        handlers::set_yield_bps(ctx, annual_yield_bps)
    }

    pub fn create_round(ctx: Context<CreateRound>, id: u64, args: CreateRoundArgs) -> Result<()> {
        handlers::create_round(ctx, id, args)
    }

    pub fn join(ctx: Context<Join>) -> Result<()> {
        handlers::join(ctx)
    }

    pub fn start_round(ctx: Context<StartRound>) -> Result<()> {
        handlers::start_round(ctx)
    }

    pub fn cancel_round(ctx: Context<CancelRound>) -> Result<()> {
        handlers::cancel_round(ctx)
    }

    pub fn commit_window(ctx: Context<CommitWindow>, seed: [u8; 32]) -> Result<()> {
        handlers::commit_window(ctx, seed)
    }

    pub fn commit_window_orao(ctx: Context<CommitOrao>, seed: [u8; 32]) -> Result<()> {
        handlers::commit_window_orao(ctx, seed)
    }

    pub fn reveal_window_demo(ctx: Context<RevealDemo>) -> Result<()> {
        handlers::reveal_window_demo(ctx)
    }

    pub fn reveal_window_orao(ctx: Context<RevealOrao>) -> Result<()> {
        handlers::reveal_window_orao(ctx)
    }

    pub fn check_in(ctx: Context<CheckIn>) -> Result<()> {
        handlers::check_in(ctx)
    }

    pub fn arm_shield(ctx: Context<ArmShield>) -> Result<()> {
        handlers::arm_shield(ctx)
    }

    pub fn sabotage(ctx: Context<Sabotage>) -> Result<()> {
        handlers::sabotage(ctx)
    }

    pub fn resolve_absence(ctx: Context<ResolveAbsence>) -> Result<()> {
        handlers::resolve_absence(ctx)
    }

    pub fn finalize_window(ctx: Context<FinalizeWindow>) -> Result<()> {
        handlers::finalize_window(ctx)
    }

    pub fn accrue_yield(ctx: Context<AccrueYield>) -> Result<()> {
        handlers::accrue_yield(ctx)
    }

    pub fn refund_stalled(ctx: Context<RefundStalled>) -> Result<()> {
        handlers::refund_stalled(ctx)
    }

    pub fn withdraw_principal(ctx: Context<WithdrawPrincipal>) -> Result<()> {
        handlers::withdraw_principal(ctx)
    }

    pub fn claim_prize(ctx: Context<ClaimPrize>) -> Result<()> {
        handlers::claim_prize(ctx)
    }

    pub fn sweep_dust(ctx: Context<SweepDust>) -> Result<()> {
        handlers::sweep_dust(ctx)
    }
}

#[derive(Accounts)]
#[instruction(annual_yield_bps: u16)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        init,
        payer = admin,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        init,
        payer = admin,
        space = 8 + YieldReserve::INIT_SPACE,
        seeds = [YIELD_SEED],
        bump
    )]
    pub yield_reserve: Account<'info, YieldReserve>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FundReserve<'info> {
    #[account(mut)]
    pub sponsor: Signer<'info>,
    #[account(mut, seeds = [YIELD_SEED], bump = yield_reserve.bump)]
    pub yield_reserve: Account<'info, YieldReserve>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetYieldBps<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Account<'info, Config>,
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct CreateRound<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        init,
        payer = authority,
        space = 8 + Round::INIT_SPACE,
        seeds = [ROUND_SEED, &id.to_le_bytes()],
        bump
    )]
    pub round: Box<Account<'info, Round>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Join<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(
        init,
        payer = user,
        space = 8 + Player::INIT_SPACE,
        seeds = [PLAYER_SEED, round.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StartRound<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(mut, seeds = [YIELD_SEED], bump = yield_reserve.bump)]
    pub yield_reserve: Account<'info, YieldReserve>,
}

#[derive(Accounts)]
pub struct CancelRound<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
}

#[derive(Accounts)]
pub struct CommitWindow<'info> {
    pub crank: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
}

#[derive(Accounts)]
pub struct CommitOrao<'info> {
    pub crank: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    /// CHECK: ORAO request PDA. Owner, address, and pending layout are checked.
    pub vrf_request: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct RevealDemo<'info> {
    pub crank: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    /// CHECK: SlotHashes sysvar. Address is constrained.
    #[account(address = anchor_lang::solana_program::sysvar::slot_hashes::ID)]
    pub slot_hashes: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct RevealOrao<'info> {
    pub crank: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    /// CHECK: ORAO request PDA. Owner, address, and fulfilled layout are checked.
    pub vrf_request: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct CheckIn<'info> {
    pub user: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(
        mut,
        seeds = [PLAYER_SEED, round.key().as_ref(), user.key().as_ref()],
        bump = player.bump
    )]
    pub player: Account<'info, Player>,
}

#[derive(Accounts)]
pub struct ArmShield<'info> {
    pub user: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(
        mut,
        seeds = [PLAYER_SEED, round.key().as_ref(), user.key().as_ref()],
        bump = player.bump
    )]
    pub player: Account<'info, Player>,
}

#[derive(Accounts)]
pub struct Sabotage<'info> {
    pub attacker: Signer<'info>,
    /// CHECK: target wallet. The player PDA is derived from this key.
    pub target: UncheckedAccount<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(
        mut,
        seeds = [PLAYER_SEED, round.key().as_ref(), attacker.key().as_ref()],
        bump = attacker_player.bump
    )]
    pub attacker_player: Account<'info, Player>,
    #[account(
        mut,
        seeds = [PLAYER_SEED, round.key().as_ref(), target.key().as_ref()],
        bump = target_player.bump
    )]
    pub target_player: Account<'info, Player>,
}

#[derive(Accounts)]
pub struct ResolveAbsence<'info> {
    pub crank: Signer<'info>,
    /// CHECK: player wallet, used to derive the player PDA.
    pub user: UncheckedAccount<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(
        mut,
        seeds = [PLAYER_SEED, round.key().as_ref(), user.key().as_ref()],
        bump = player.bump
    )]
    pub player: Account<'info, Player>,
}

#[derive(Accounts)]
pub struct FinalizeWindow<'info> {
    pub crank: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(mut, seeds = [YIELD_SEED], bump = yield_reserve.bump)]
    pub yield_reserve: Account<'info, YieldReserve>,
}

#[derive(Accounts)]
pub struct AccrueYield<'info> {
    pub crank: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(mut, seeds = [YIELD_SEED], bump = yield_reserve.bump)]
    pub yield_reserve: Account<'info, YieldReserve>,
}

#[derive(Accounts)]
pub struct RefundStalled<'info> {
    pub crank: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(mut, seeds = [YIELD_SEED], bump = yield_reserve.bump)]
    pub yield_reserve: Account<'info, YieldReserve>,
}

#[derive(Accounts)]
pub struct WithdrawPrincipal<'info> {
    pub crank: Signer<'info>,
    /// CHECK: deposit is returned to this wallet. It must be the player record's user.
    #[account(mut)]
    pub user: UncheckedAccount<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(
        mut,
        seeds = [PLAYER_SEED, round.key().as_ref(), user.key().as_ref()],
        bump = player.bump
    )]
    pub player: Account<'info, Player>,
}

#[derive(Accounts)]
pub struct ClaimPrize<'info> {
    pub crank: Signer<'info>,
    /// CHECK: prize is paid to this wallet. It must be the player record's user.
    #[account(mut)]
    pub user: UncheckedAccount<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(
        mut,
        seeds = [PLAYER_SEED, round.key().as_ref(), user.key().as_ref()],
        bump = player.bump
    )]
    pub player: Account<'info, Player>,
}

#[derive(Accounts)]
pub struct SweepDust<'info> {
    pub crank: Signer<'info>,
    #[account(mut, seeds = [ROUND_SEED, &round.id.to_le_bytes()], bump = round.bump)]
    pub round: Box<Account<'info, Round>>,
    #[account(mut, seeds = [YIELD_SEED], bump = yield_reserve.bump)]
    pub yield_reserve: Account<'info, YieldReserve>,
}

mod handlers {
    use super::*;
    use crate::randomness::{
        assert_orao_pda, demo_randomness, gap_seconds, parse_orao, read_slot_hash,
    };
    use crate::yield_math::simulated_yield;

    pub fn initialize(ctx: Context<Initialize>, annual_yield_bps: u16) -> Result<()> {
        require!(
            (1..=10_000).contains(&annual_yield_bps),
            LastSaverError::BadYieldRate
        );
        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.annual_yield_bps = annual_yield_bps;
        config.yield_source = YIELD_SOURCE_MOCK_RESERVE;
        config.next_round_id = 0;
        config.bump = ctx.bumps.config;
        let reserve = &mut ctx.accounts.yield_reserve;
        reserve.admin = ctx.accounts.admin.key();
        reserve.bump = ctx.bumps.yield_reserve;
        Ok(())
    }

    pub fn fund_reserve(ctx: Context<FundReserve>, amount: u64) -> Result<()> {
        require!(amount > 0, LastSaverError::BadConfig);
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.sponsor.to_account_info(),
                    to: ctx.accounts.yield_reserve.to_account_info(),
                },
            ),
            amount,
        )?;
        Ok(())
    }

    pub fn set_yield_bps(ctx: Context<SetYieldBps>, annual_yield_bps: u16) -> Result<()> {
        require!(
            (1..=10_000).contains(&annual_yield_bps),
            LastSaverError::BadYieldRate
        );
        ctx.accounts.config.annual_yield_bps = annual_yield_bps;
        Ok(())
    }

    pub fn create_round(ctx: Context<CreateRound>, id: u64, args: CreateRoundArgs) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        validate_create(&args, now)?;
        require!(
            id == ctx.accounts.config.next_round_id,
            LastSaverError::BadRoundId
        );
        ctx.accounts.config.next_round_id = ctx
            .accounts
            .config
            .next_round_id
            .checked_add(1)
            .ok_or(LastSaverError::MathOverflow)?;

        let bump = ctx.bumps.round;
        let authority = ctx.accounts.authority.key();
        let round = &mut ctx.accounts.round;
        round.id = id;
        round.authority = authority;
        round.entry_lamports = args.entry_lamports;
        round.sponsor_bonus = args.sponsor_bonus;
        round.max_players = args.max_players;
        round.min_players = args.min_players;
        round.standards_scheduled = args.windows;
        round.window_duration = args.window_duration;
        round.min_gap = args.min_gap;
        round.max_gap = args.max_gap;
        round.stall_timeout = args.stall_timeout;
        round.join_until = args.join_until;
        round.mode = args.mode;
        round.status = RoundStatus::Forming;
        round.pending_kind = WindowKind::Standard;
        round.current_kind = WindowKind::None;
        round.bump = bump;
        check_round(round)?;
        Ok(())
    }

    pub fn join(ctx: Context<Join>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let entry = {
            let round = &ctx.accounts.round;
            require!(
                round.status == RoundStatus::Forming,
                LastSaverError::NotForming
            );
            require!(now < round.join_until, LastSaverError::JoinClosed);
            require!(
                round.player_count < round.max_players,
                LastSaverError::RoundFull
            );
            round.entry_lamports
        };

        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.user.to_account_info(),
                    to: ctx.accounts.round.to_account_info(),
                },
            ),
            entry,
        )?;

        let user = ctx.accounts.user.key();
        let bump = ctx.bumps.player;
        let round_key = ctx.accounts.round.key();
        let (round_id, player_count) = {
            let round = &mut ctx.accounts.round;
            let idx = round.player_count as usize;
            round.players[idx] = user;
            round.player_count = round
                .player_count
                .checked_add(1)
                .ok_or(LastSaverError::MathOverflow)?;
            round.alive_count = round
                .alive_count
                .checked_add(1)
                .ok_or(LastSaverError::MathOverflow)?;
            round.principal_lamports = round
                .principal_lamports
                .checked_add(entry)
                .ok_or(LastSaverError::MathOverflow)?;
            check_round(round)?;
            (round.id, round.player_count)
        };

        let player = &mut ctx.accounts.player;
        player.round = round_key;
        player.user = user;
        player.alive = true;
        player.shield_charges = 1;
        player.sabotage_charges = 1;
        player.bump = bump;

        emit!(Joined {
            round_id,
            user,
            player_count,
        });
        Ok(())
    }

    pub fn start_round(ctx: Context<StartRound>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let bonus = ctx.accounts.round.sponsor_bonus;
        let bps = ctx.accounts.config.annual_yield_bps;
        let source = ctx.accounts.config.yield_source;
        {
            let round = &ctx.accounts.round;
            require!(
                round.status == RoundStatus::Forming,
                LastSaverError::NotForming
            );
            require!(
                round.player_count >= round.min_players,
                LastSaverError::NotEnoughPlayers
            );
            if now < round.join_until {
                require_keys_eq!(
                    ctx.accounts.authority.key(),
                    round.authority,
                    LastSaverError::EarlyStart
                );
            }
        }
        if bonus > 0 {
            move_lamports(
                &ctx.accounts.yield_reserve.to_account_info(),
                &ctx.accounts.round.to_account_info(),
                bonus,
            )?;
        }
        let stall = ctx.accounts.round.stall_timeout;
        let round = &mut ctx.accounts.round;
        round.prize_lamports = bonus;
        round.yield_bps = bps;
        round.yield_source = source;
        round.started_at = now;
        round.last_yield_ts = now;
        round.progress_deadline = now.checked_add(stall).ok_or(LastSaverError::MathOverflow)?;
        round.status = RoundStatus::NeedCommit;
        round.pending_kind = WindowKind::Standard;
        check_round(round)?;
        Ok(())
    }

    pub fn cancel_round(ctx: Context<CancelRound>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let round = &mut ctx.accounts.round;
        require!(
            round.status == RoundStatus::Forming,
            LastSaverError::BadPhase
        );
        let expired = now >= round.join_until && round.player_count < round.min_players;
        let is_authority = ctx.accounts.authority.key() == round.authority;
        require!(is_authority || expired, LastSaverError::CannotCancel);
        round.status = RoundStatus::Cancelled;
        round.settled_at = now;
        Ok(())
    }

    pub fn commit_window(ctx: Context<CommitWindow>, seed: [u8; 32]) -> Result<()> {
        let clock = Clock::get()?;
        let round = &mut ctx.accounts.round;
        require!(
            round.mode == RandomnessMode::DemoSlot,
            LastSaverError::WrongMode
        );
        seal_commit(round, seed, clock.slot, clock.unix_timestamp)?;
        Ok(())
    }

    pub fn commit_window_orao(ctx: Context<CommitOrao>, seed: [u8; 32]) -> Result<()> {
        let clock = Clock::get()?;
        require!(
            ctx.accounts.round.mode == RandomnessMode::Orao,
            LastSaverError::WrongMode
        );
        require!(
            ctx.accounts.round.status == RoundStatus::NeedCommit,
            LastSaverError::BadPhase
        );
        require!(seed != [0u8; 32], LastSaverError::BadSeed);
        assert_orao_pda(&seed, &ctx.accounts.vrf_request.to_account_info())?;
        let view = {
            let data = ctx.accounts.vrf_request.try_borrow_data()?;
            parse_orao(&data)?
        };
        require!(view.seed == seed, LastSaverError::VrfSeedMismatch);
        require!(
            view.randomness.is_none(),
            LastSaverError::VrfAlreadyFulfilled
        );
        let round = &mut ctx.accounts.round;
        seal_commit(round, seed, clock.slot, clock.unix_timestamp)?;
        Ok(())
    }

    pub fn reveal_window_demo(ctx: Context<RevealDemo>) -> Result<()> {
        let clock = Clock::get()?;
        require!(
            ctx.accounts.round.mode == RandomnessMode::DemoSlot,
            LastSaverError::WrongMode
        );
        require!(
            ctx.accounts.round.status == RoundStatus::Committed,
            LastSaverError::BadPhase
        );
        require!(
            clock.slot > ctx.accounts.round.commit_slot,
            LastSaverError::SlotHashUnavailable
        );
        let commit_slot = ctx.accounts.round.commit_slot;
        let seed = ctx.accounts.round.commit_seed;
        let next_window = ctx
            .accounts
            .round
            .window_id
            .checked_add(1)
            .ok_or(LastSaverError::MathOverflow)?;
        let slot_hash = {
            let data = ctx.accounts.slot_hashes.try_borrow_data()?;
            read_slot_hash(&data, commit_slot).ok_or(LastSaverError::SlotHashUnavailable)?
        };
        let randomness = demo_randomness(&slot_hash, &seed, next_window);
        open_from_randomness(&mut ctx.accounts.round, randomness, clock.unix_timestamp)?;
        Ok(())
    }

    pub fn reveal_window_orao(ctx: Context<RevealOrao>) -> Result<()> {
        let clock = Clock::get()?;
        require!(
            ctx.accounts.round.mode == RandomnessMode::Orao,
            LastSaverError::WrongMode
        );
        require!(
            ctx.accounts.round.status == RoundStatus::Committed,
            LastSaverError::BadPhase
        );
        let seed = ctx.accounts.round.commit_seed;
        assert_orao_pda(&seed, &ctx.accounts.vrf_request.to_account_info())?;
        let randomness = {
            let data = ctx.accounts.vrf_request.try_borrow_data()?;
            let view = parse_orao(&data)?;
            require!(view.seed == seed, LastSaverError::VrfSeedMismatch);
            view.randomness.ok_or(LastSaverError::VrfPending)?
        };
        open_from_randomness(&mut ctx.accounts.round, randomness, clock.unix_timestamp)?;
        Ok(())
    }

    pub fn check_in(ctx: Context<CheckIn>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_in_window(&ctx.accounts.round, now)?;
        let window_id = ctx.accounts.round.window_id;
        let kind = ctx.accounts.round.current_kind;
        let round = &mut ctx.accounts.round;
        let player = &mut ctx.accounts.player;
        require_attending(round, player)?;
        player.checked_window = window_id;
        if player.shield_armed_for == window_id {
            player.shield_armed_for = 0;
        }
        if kind == WindowKind::Jolt {
            clear_jolt(round, player);
        }
        round.attendees_remaining = round
            .attendees_remaining
            .checked_sub(1)
            .ok_or(LastSaverError::MathOverflow)?;
        Ok(())
    }

    pub fn arm_shield(ctx: Context<ArmShield>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_in_window(&ctx.accounts.round, now)?;
        let window_id = ctx.accounts.round.window_id;
        let round = &mut ctx.accounts.round;
        let player = &mut ctx.accounts.player;
        require_attending(round, player)?;
        require!(player.shield_charges > 0, LastSaverError::NoShield);
        player.shield_armed_for = window_id;
        Ok(())
    }

    pub fn sabotage(ctx: Context<Sabotage>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_in_window(&ctx.accounts.round, now)?;
        require!(
            ctx.accounts.round.current_kind == WindowKind::Standard,
            LastSaverError::JoltClosed
        );
        require!(
            ctx.accounts.attacker.key() != ctx.accounts.target.key(),
            LastSaverError::SelfSabotage
        );
        let round = &mut ctx.accounts.round;
        let attacker = &mut ctx.accounts.attacker_player;
        let target = &mut ctx.accounts.target_player;
        require!(attacker.alive, LastSaverError::Eliminated);
        require!(attacker.sabotage_charges > 0, LastSaverError::NoSabotage);
        require!(target.alive, LastSaverError::CannotJolt);
        require!(target.jolts_received == 0, LastSaverError::CannotJolt);
        require!(!target.jolt_pending, LastSaverError::CannotJolt);
        attacker.sabotage_charges -= 1;
        target.jolt_pending = true;
        target.jolts_received = 1;
        round.jolt_marks = round
            .jolt_marks
            .checked_add(1)
            .ok_or(LastSaverError::MathOverflow)?;
        emit!(PlayerJolted {
            round_id: round.id,
            attacker: attacker.user,
            target: target.user,
        });
        Ok(())
    }

    pub fn resolve_absence(ctx: Context<ResolveAbsence>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(
            ctx.accounts.round.status == RoundStatus::Open,
            LastSaverError::BadPhase
        );
        require!(now >= ctx.accounts.round.close_at, LastSaverError::BadPhase);
        let window_id = ctx.accounts.round.window_id;
        let kind = ctx.accounts.round.current_kind;
        let round_id = ctx.accounts.round.id;
        let round = &mut ctx.accounts.round;
        let player = &mut ctx.accounts.player;
        require!(player.alive, LastSaverError::Eliminated);
        require!(
            player.checked_window != window_id,
            LastSaverError::AlreadyCheckedIn
        );
        require!(
            player.resolved_window != window_id,
            LastSaverError::AlreadyResolved
        );
        match kind {
            WindowKind::Standard => {}
            WindowKind::Jolt => require!(player.jolt_pending, LastSaverError::NotRequired),
            WindowKind::None => return err!(LastSaverError::BadPhase),
        }

        let shielded = player.shield_armed_for == window_id && player.shield_charges > 0;
        let user = player.user;
        if shielded {
            player.shield_charges -= 1;
            player.shield_armed_for = 0;
            emit!(ShieldConsumed {
                round_id,
                user,
                window_id,
            });
        }
        player.resolved_window = window_id;
        if kind == WindowKind::Jolt {
            clear_jolt(round, player);
        } else if !shielded && player.jolt_pending {
            clear_jolt(round, player);
        }
        if !shielded {
            player.alive = false;
            round.alive_count = round
                .alive_count
                .checked_sub(1)
                .ok_or(LastSaverError::MathOverflow)?;
            emit!(PlayerEliminated {
                round_id,
                user,
                window_id,
            });
        }
        round.attendees_remaining = round
            .attendees_remaining
            .checked_sub(1)
            .ok_or(LastSaverError::MathOverflow)?;
        Ok(())
    }

    pub fn finalize_window(ctx: Context<FinalizeWindow>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let action = {
            let round = &ctx.accounts.round;
            require!(round.status == RoundStatus::Open, LastSaverError::BadPhase);
            require!(now >= round.close_at, LastSaverError::BadPhase);
            require!(
                round.attendees_remaining == 0,
                LastSaverError::UnresolvedPlayers
            );
            if round.alive_count <= 1 {
                Next::Settle
            } else if round.current_kind == WindowKind::Jolt || round.jolt_marks == 0 {
                if round.standards_opened >= round.standards_scheduled {
                    Next::Settle
                } else {
                    Next::Standard
                }
            } else {
                Next::Jolt
            }
        };
        let reserve = ctx.accounts.yield_reserve.to_account_info();
        match action {
            Next::Settle => settle(&mut ctx.accounts.round, &reserve, now)?,
            Next::Standard => arm_next(&mut ctx.accounts.round, WindowKind::Standard, now)?,
            Next::Jolt => arm_next(&mut ctx.accounts.round, WindowKind::Jolt, now)?,
        }
        Ok(())
    }

    pub fn accrue_yield(ctx: Context<AccrueYield>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(is_live(ctx.accounts.round.status), LastSaverError::BadPhase);
        let reserve = ctx.accounts.yield_reserve.to_account_info();
        accrue(&mut ctx.accounts.round, &reserve, now)?;
        check_round(&ctx.accounts.round)?;
        Ok(())
    }

    pub fn refund_stalled(ctx: Context<RefundStalled>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(is_live(ctx.accounts.round.status), LastSaverError::BadPhase);
        require!(
            now > ctx.accounts.round.progress_deadline,
            LastSaverError::NotStalled
        );
        let reserve = ctx.accounts.yield_reserve.to_account_info();
        let round = &mut ctx.accounts.round;
        return_prize(round, &reserve)?;
        round.winners = 0;
        round.status = RoundStatus::Cancelled;
        round.settled_at = now;
        check_round(round)?;
        Ok(())
    }

    pub fn withdraw_principal(ctx: Context<WithdrawPrincipal>) -> Result<()> {
        require!(
            ctx.accounts.round.status == RoundStatus::Settled
                || ctx.accounts.round.status == RoundStatus::Cancelled,
            LastSaverError::RoundStillActive
        );
        require!(
            !ctx.accounts.player.principal_withdrawn,
            LastSaverError::AlreadyWithdrawn
        );
        require_keys_eq!(
            ctx.accounts.player.user,
            ctx.accounts.user.key(),
            LastSaverError::BadPhase
        );
        let amount = ctx.accounts.round.entry_lamports;
        ctx.accounts.player.principal_withdrawn = true;
        ctx.accounts.round.principal_lamports = ctx
            .accounts
            .round
            .principal_lamports
            .checked_sub(amount)
            .ok_or(LastSaverError::MathOverflow)?;
        move_lamports(
            &ctx.accounts.round.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            amount,
        )?;
        check_round(&ctx.accounts.round)?;
        Ok(())
    }

    pub fn claim_prize(ctx: Context<ClaimPrize>) -> Result<()> {
        require!(
            ctx.accounts.round.status == RoundStatus::Settled,
            LastSaverError::NotSettled
        );
        require!(ctx.accounts.player.alive, LastSaverError::Eliminated);
        require!(
            !ctx.accounts.player.prize_claimed,
            LastSaverError::AlreadyClaimed
        );
        require!(
            ctx.accounts.round.prize_per_winner > 0,
            LastSaverError::NoPrize
        );
        require_keys_eq!(
            ctx.accounts.player.user,
            ctx.accounts.user.key(),
            LastSaverError::BadPhase
        );
        let amount = ctx.accounts.round.prize_per_winner;
        ctx.accounts.player.prize_claimed = true;
        ctx.accounts.round.prize_lamports = ctx
            .accounts
            .round
            .prize_lamports
            .checked_sub(amount)
            .ok_or(LastSaverError::MathOverflow)?;
        move_lamports(
            &ctx.accounts.round.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            amount,
        )?;
        check_round(&ctx.accounts.round)?;
        Ok(())
    }

    pub fn sweep_dust(ctx: Context<SweepDust>) -> Result<()> {
        require!(
            ctx.accounts.round.status == RoundStatus::Settled
                || ctx.accounts.round.status == RoundStatus::Cancelled,
            LastSaverError::BadPhase
        );
        let dust = ctx.accounts.round.prize_dust;
        require!(dust > 0, LastSaverError::NoDust);
        ctx.accounts.round.prize_dust = 0;
        move_lamports(
            &ctx.accounts.round.to_account_info(),
            &ctx.accounts.yield_reserve.to_account_info(),
            dust,
        )?;
        check_round(&ctx.accounts.round)?;
        Ok(())
    }

    enum Next {
        Settle,
        Standard,
        Jolt,
    }

    fn validate_create(args: &CreateRoundArgs, now: i64) -> Result<()> {
        require!(
            (1_000_000..=1_000 * 1_000_000_000).contains(&args.entry_lamports),
            LastSaverError::BadConfig
        );
        require!(
            args.sponsor_bonus <= 1_000 * 1_000_000_000,
            LastSaverError::BadConfig
        );
        require!(args.min_players >= 2, LastSaverError::BadConfig);
        require!(
            args.max_players >= args.min_players && args.max_players as usize <= MAX_PLAYERS,
            LastSaverError::BadConfig
        );
        require!((1..=12).contains(&args.windows), LastSaverError::BadConfig);
        require!(
            (5..=86_400).contains(&args.window_duration),
            LastSaverError::BadConfig
        );
        require!(
            args.min_gap >= 0 && args.max_gap >= args.min_gap && args.max_gap <= 86_400,
            LastSaverError::BadConfig
        );
        let min_stall = args
            .window_duration
            .checked_add(args.max_gap)
            .and_then(|v| v.checked_add(60))
            .ok_or(LastSaverError::MathOverflow)?;
        require!(
            args.stall_timeout >= min_stall && args.stall_timeout <= 7 * 86_400,
            LastSaverError::BadConfig
        );
        require!(
            args.join_until > now && args.join_until <= now + 14 * 86_400,
            LastSaverError::BadConfig
        );
        Ok(())
    }

    fn seal_commit(round: &mut Account<Round>, seed: [u8; 32], slot: u64, now: i64) -> Result<()> {
        require!(
            round.status == RoundStatus::NeedCommit,
            LastSaverError::BadPhase
        );
        require!(seed != [0u8; 32], LastSaverError::BadSeed);
        round.commit_seed = seed;
        round.commit_slot = slot;
        round.commit_ts = now;
        round.progress_deadline = now
            .checked_add(round.stall_timeout)
            .ok_or(LastSaverError::MathOverflow)?;
        round.status = RoundStatus::Committed;
        Ok(())
    }

    fn open_from_randomness(
        round: &mut Account<Round>,
        randomness: [u8; 64],
        now: i64,
    ) -> Result<()> {
        require!(
            round.status == RoundStatus::Committed,
            LastSaverError::BadPhase
        );
        let gap = gap_seconds(&randomness, round.min_gap, round.max_gap);
        let open_at = now.checked_add(gap).ok_or(LastSaverError::MathOverflow)?;
        let close_at = open_at
            .checked_add(round.window_duration)
            .ok_or(LastSaverError::MathOverflow)?;
        round.window_id = round
            .window_id
            .checked_add(1)
            .ok_or(LastSaverError::MathOverflow)?;
        round.current_kind = round.pending_kind;
        round.open_at = open_at;
        round.close_at = close_at;
        round.last_seed = round.commit_seed;
        round.last_randomness = randomness;
        round.last_gap = gap;
        round.status = RoundStatus::Open;
        round.attendees_remaining = match round.current_kind {
            WindowKind::Standard => {
                round.standards_opened = round
                    .standards_opened
                    .checked_add(1)
                    .ok_or(LastSaverError::MathOverflow)?;
                round.alive_count
            }
            WindowKind::Jolt => round.jolt_marks,
            WindowKind::None => return err!(LastSaverError::BadPhase),
        };
        require!(round.attendees_remaining > 0, LastSaverError::BadPhase);
        round.progress_deadline = close_at
            .checked_add(round.stall_timeout)
            .ok_or(LastSaverError::MathOverflow)?;
        emit!(WindowOpened {
            round_id: round.id,
            window_id: round.window_id,
            kind: round.current_kind as u8,
            open_at,
            close_at,
            gap,
        });
        Ok(())
    }

    fn require_in_window(round: &Round, now: i64) -> Result<()> {
        require!(round.status == RoundStatus::Open, LastSaverError::BadPhase);
        require!(
            now >= round.open_at && now < round.close_at,
            LastSaverError::WindowClosed
        );
        Ok(())
    }

    fn require_attending(round: &Round, player: &Player) -> Result<()> {
        require!(player.alive, LastSaverError::Eliminated);
        require!(
            player.checked_window != round.window_id,
            LastSaverError::AlreadyCheckedIn
        );
        require!(
            player.resolved_window != round.window_id,
            LastSaverError::AlreadyResolved
        );
        match round.current_kind {
            WindowKind::Standard => Ok(()),
            WindowKind::Jolt => {
                require!(player.jolt_pending, LastSaverError::NotRequired);
                Ok(())
            }
            WindowKind::None => err!(LastSaverError::BadPhase),
        }
    }

    fn clear_jolt(round: &mut Round, player: &mut Player) {
        if player.jolt_pending {
            player.jolt_pending = false;
            round.jolt_marks = round.jolt_marks.saturating_sub(1);
        }
    }

    fn is_live(status: RoundStatus) -> bool {
        matches!(
            status,
            RoundStatus::NeedCommit | RoundStatus::Committed | RoundStatus::Open
        )
    }

    fn arm_next(round: &mut Account<Round>, kind: WindowKind, now: i64) -> Result<()> {
        round.pending_kind = kind;
        round.current_kind = WindowKind::None;
        round.status = RoundStatus::NeedCommit;
        round.progress_deadline = now
            .checked_add(round.stall_timeout)
            .ok_or(LastSaverError::MathOverflow)?;
        Ok(())
    }

    fn accrue(round: &mut Account<Round>, reserve: &AccountInfo, now: i64) -> Result<u64> {
        if now <= round.last_yield_ts {
            return Ok(0);
        }
        let seconds = (now - round.last_yield_ts) as u64;
        let gross = simulated_yield(round.principal_lamports, round.yield_bps, seconds);
        let rent_min = Rent::get()?.minimum_balance(reserve.data_len());
        let liquid = reserve.lamports().saturating_sub(rent_min);
        let paid = gross.min(liquid);
        if paid > 0 {
            let reserve_left = reserve
                .lamports()
                .checked_sub(paid)
                .ok_or(LastSaverError::InsufficientVault)?;
            let round_next = round
                .to_account_info()
                .lamports()
                .checked_add(paid)
                .ok_or(LastSaverError::MathOverflow)?;
            **reserve.try_borrow_mut_lamports()? = reserve_left;
            {
                let round_ai = round.to_account_info();
                **round_ai.try_borrow_mut_lamports()? = round_next;
            }
            round.prize_lamports = round
                .prize_lamports
                .checked_add(paid)
                .ok_or(LastSaverError::MathOverflow)?;
            round.yield_accrued = round
                .yield_accrued
                .checked_add(paid)
                .ok_or(LastSaverError::MathOverflow)?;
        }
        round.last_yield_ts = now;
        Ok(paid)
    }

    fn settle(round: &mut Account<Round>, reserve: &AccountInfo, now: i64) -> Result<()> {
        accrue(round, reserve, now)?;
        round.settled_at = now;
        round.status = RoundStatus::Settled;
        round.winners = round.alive_count;
        if round.alive_count == 0 {
            return_prize(round, reserve)?;
        } else {
            let winners = round.alive_count as u64;
            let per = round.prize_lamports / winners;
            let dust = round.prize_lamports % winners;
            round.prize_dust = round
                .prize_dust
                .checked_add(dust)
                .ok_or(LastSaverError::MathOverflow)?;
            round.prize_per_winner = per;
            round.prize_lamports = per
                .checked_mul(winners)
                .ok_or(LastSaverError::MathOverflow)?;
        }
        check_round(round)?;
        emit!(RoundSettled {
            round_id: round.id,
            winners: round.winners,
            prize_per_winner: round.prize_per_winner,
            yield_accrued: round.yield_accrued,
            yield_source: round.yield_source,
            prize_refunded: round.prize_refunded,
        });
        Ok(())
    }

    fn return_prize(round: &mut Account<Round>, reserve: &AccountInfo) -> Result<()> {
        let amount = round
            .prize_lamports
            .checked_add(round.prize_dust)
            .ok_or(LastSaverError::MathOverflow)?;
        if amount > 0 {
            let ai = round.to_account_info();
            move_lamports(&ai, reserve, amount)?;
        }
        round.prize_lamports = 0;
        round.prize_dust = 0;
        round.prize_per_winner = 0;
        round.prize_refunded = true;
        Ok(())
    }

    fn move_lamports(from: &AccountInfo, to: &AccountInfo, amount: u64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }
        let rent_min = Rent::get()?.minimum_balance(from.data_len());
        let left = from
            .lamports()
            .checked_sub(amount)
            .ok_or(LastSaverError::InsufficientVault)?;
        require!(left >= rent_min, LastSaverError::InsufficientVault);
        let to_next = to
            .lamports()
            .checked_add(amount)
            .ok_or(LastSaverError::MathOverflow)?;
        **from.try_borrow_mut_lamports()? = left;
        **to.try_borrow_mut_lamports()? = to_next;
        Ok(())
    }

    fn assert_backing(ai: &AccountInfo, round: &Round) -> Result<()> {
        let rent_min = Rent::get()?.minimum_balance(ai.data_len());
        let need = rent_min
            .checked_add(round.principal_lamports)
            .and_then(|v| v.checked_add(round.prize_lamports))
            .and_then(|v| v.checked_add(round.prize_dust))
            .ok_or(LastSaverError::MathOverflow)?;
        require!(ai.lamports() >= need, LastSaverError::Accounting);
        Ok(())
    }

    fn check_round(round: &Account<Round>) -> Result<()> {
        let ai = round.to_account_info();
        assert_backing(&ai, round)
    }
}
