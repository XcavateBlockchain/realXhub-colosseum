use anchor_lang::prelude::*;

use crate::constants::{CPI_AUTH_SEED, PROPERTY_PROGRAM, SHARE_SEED};
use crate::error::MarketplaceError;
use crate::state::{LockReason, ShareHolding};

/// Share-lock bookkeeping for the property program's elections. The votes
/// live over there, but a lock only blocks transfers if it sits on the
/// ShareHolding this program checks, so the property program mirrors its
/// vote locks in through these two instructions. Callable only via CPI: the
/// signer is a PDA of the property program, so no wallet can ever produce
/// the signature.
#[derive(Accounts)]
#[instruction(asset_id: u64, owner: Pubkey)]
pub struct AdjustShareLock<'info> {
    /// The property program's CPI signer.
    #[account(seeds = [CPI_AUTH_SEED], bump, seeds::program = PROPERTY_PROGRAM)]
    pub property_signer: Signer<'info>,

    #[account(
        mut,
        seeds = [SHARE_SEED, &asset_id.to_le_bytes(), owner.as_ref()],
        bump = holding.bump,
    )]
    pub holding: Box<Account<'info, ShareHolding>>,
}

pub fn lock_shares_handler(
    ctx: Context<AdjustShareLock>,
    _asset_id: u64,
    _owner: Pubkey,
    reason: LockReason,
    amount: u32,
) -> Result<()> {
    let holding = &mut ctx.accounts.holding;
    let locked_after = holding.locks[reason as usize]
        .checked_add(amount)
        .ok_or(MarketplaceError::Overflow)?;
    // Each reason is capped by the votable balance on its own; other
    // reasons don't count against it, but listed shares do.
    require!(
        locked_after <= holding.votable(),
        MarketplaceError::NotEnoughShares
    );
    holding.locks[reason as usize] = locked_after;
    Ok(())
}

pub fn unlock_shares_handler(
    ctx: Context<AdjustShareLock>,
    _asset_id: u64,
    _owner: Pubkey,
    reason: LockReason,
    amount: u32,
) -> Result<()> {
    let holding = &mut ctx.accounts.holding;
    holding.locks[reason as usize] = holding.locks[reason as usize]
        .checked_sub(amount)
        .ok_or(MarketplaceError::Overflow)?;
    Ok(())
}
