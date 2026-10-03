use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::constants::*;
use crate::error::HubError;
use crate::state::{Config, Hub, HubFunding, HubStatus, ReservationSale};

/// Also supports reservation hubs funded before automatic tranche release was
/// installed. Anyone may pay the transaction costs; the recipient is fixed.
#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct ReleaseFirstTranche<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(seeds = [RESERVATION_SALE_SEED, &hub_id.to_le_bytes()], bump = sale.bump,
        has_one = hub @ HubError::InvalidPosition,
        constraint = sale.reserved_tokens == 0 @ HubError::UnpaidReservationsOutstanding)]
    pub sale: Box<Account<'info, ReservationSale>>,
    #[account(address = config.payment_mint @ HubError::InvalidMint)]
    pub payment_mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [PAYMENT_VAULT_SEED, &hub_id.to_le_bytes()], bump,
        token::mint = payment_mint, token::authority = hub)]
    pub payment_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: Only the recorded operator may be the associated token recipient.
    #[account(address = hub.operator @ HubError::NotOperator)]
    pub operator: UncheckedAccount<'info>,
    #[account(init_if_needed, payer = cranker,
        associated_token::mint = payment_mint, associated_token::authority = operator)]
    pub operator_payment: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed, payer = cranker, space = 8 + HubFunding::INIT_SPACE,
        seeds = [FUNDING_SEED, &hub_id.to_le_bytes()], bump)]
    pub funding: Box<Account<'info, HubFunding>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn release_first_tranche_handler(
    ctx: Context<ReleaseFirstTranche>,
    _hub_id: u64,
) -> Result<()> {
    release_first_tranche(
        &ctx.accounts.hub,
        &mut ctx.accounts.funding,
        ctx.bumps.funding,
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.payment_vault.to_account_info(),
                mint: ctx.accounts.payment_mint.to_account_info(),
                to: ctx.accounts.operator_payment.to_account_info(),
                authority: ctx.accounts.hub.to_account_info(),
            },
        ),
        ctx.accounts.payment_mint.decimals,
        Clock::get()?.unix_timestamp,
    )
}

/// Amounts come from recorded buyer payments, so direct vault donations cannot
/// increase the operator's entitlement. The receipt survives a retry or upgrade.
pub(super) fn release_first_tranche<'info>(
    hub: &Account<'info, Hub>,
    funding: &mut HubFunding,
    funding_bump: u8,
    transfer: CpiContext<'_, '_, '_, 'info, TransferChecked<'info>>,
    decimals: u8,
    now: i64,
) -> Result<()> {
    require!(
        hub.status == HubStatus::Funded
            && hub.tokens_sold == hub.token_supply
            && hub.tokens_claimed == hub.token_supply
            && hub.total_refunded == 0,
        HubError::InvalidStatus
    );
    let target = hub
        .token_price
        .checked_mul(hub.token_supply)
        .ok_or(HubError::Overflow)?;
    require!(
        hub.total_paid == target && target > 0,
        HubError::InvalidPosition
    );
    require!(
        funding.hub == Pubkey::default() || funding.hub == hub.key(),
        HubError::InvalidPosition
    );
    require!(
        !funding.first_tranche_released,
        HubError::FirstTrancheAlreadyReleased
    );

    // Keep an indivisible extra payment unit in escrow for the second tranche.
    let amount = hub.total_paid / 2;
    let id_bytes = hub.hub_id.to_le_bytes();
    let bump = [hub.bump];
    let seeds: &[&[u8]] = &[HUB_SEED, &id_bytes, &bump];
    token::transfer_checked(transfer.with_signer(&[seeds]), amount, decimals)?;
    funding.hub = hub.key();
    funding.first_tranche_amount = amount;
    funding.first_tranche_released = true;
    funding.first_tranche_released_at = now;
    funding.bump = funding_bump;
    emit!(FirstTrancheReleased {
        hub_id: hub.hub_id,
        operator: hub.operator,
        amount,
        retained_payment: hub
            .total_paid
            .checked_sub(amount)
            .ok_or(HubError::Overflow)?,
        released_at: now,
    });
    Ok(())
}

#[event]
pub struct FirstTrancheReleased {
    pub hub_id: u64,
    pub operator: Pubkey,
    pub amount: u64,
    pub retained_payment: u64,
    pub released_at: i64,
}
