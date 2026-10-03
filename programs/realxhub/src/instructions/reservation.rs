use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};
use xcavate_whitelist::state::{Role, RoleAccount};

use crate::constants::*;
use crate::error::HubError;
use crate::state::{Config, Hub, HubReservation, HubStatus, PaymentReservation, ReservationSale};

#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct OpenReservations<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(mut, seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump, has_one = operator @ HubError::NotOperator)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(
        seeds = [xcavate_whitelist::ROLE_SEED, operator.key().as_ref(), &[Role::RegionalOperator.seed_byte()]],
        bump = operator_role.bump,
        seeds::program = xcavate_whitelist::ID,
        constraint = operator_role.is_compliant() @ HubError::NotCompliant,
    )]
    pub operator_role: Box<Account<'info, RoleAccount>>,
    #[account(
        seeds = [regions::REGION_SEED, &hub.region_id.to_le_bytes()],
        bump = region.bump,
        seeds::program = regions::ID,
        constraint = region.owner == operator.key() @ HubError::NotRegionOwner,
    )]
    pub region: Box<Account<'info, regions::state::Region>>,
    #[account(init, payer = operator, space = 8 + ReservationSale::INIT_SPACE, seeds = [RESERVATION_SALE_SEED, &hub_id.to_le_bytes()], bump)]
    pub sale: Box<Account<'info, ReservationSale>>,
    pub system_program: Program<'info, System>,
}

pub fn open_reservations_handler(ctx: Context<OpenReservations>, hub_id: u64) -> Result<()> {
    let hub = &mut ctx.accounts.hub;
    require!(hub.status == HubStatus::Listed, HubError::InvalidStatus);
    require!(
        Clock::get()?.unix_timestamp < hub.sale_deadline,
        HubError::SaleExpired
    );
    // Legacy paid sales keep their existing claim/refund path. A hub may only
    // switch before accepting its first payment, never after buyers commit funds.
    require!(
        hub.tokens_sold == 0 && hub.total_paid == 0 && hub.tokens_claimed == 0,
        HubError::PurchasesOutstanding
    );
    ctx.accounts.sale.hub = hub.key();
    ctx.accounts.sale.bump = ctx.bumps.sale;
    hub.status = HubStatus::Reserving;
    emit!(ReservationsOpened { hub_id });
    Ok(())
}

#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct ReserveTokens<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(
        seeds = [xcavate_whitelist::ROLE_SEED, buyer.key().as_ref(), &[Role::RealEstateInvestor.seed_byte()]],
        bump = buyer_role.bump,
        seeds::program = xcavate_whitelist::ID,
        constraint = buyer_role.is_compliant() @ HubError::NotCompliant,
    )]
    pub buyer_role: Box<Account<'info, RoleAccount>>,
    #[account(mut, seeds = [RESERVATION_SALE_SEED, &hub_id.to_le_bytes()], bump = sale.bump, has_one = hub @ HubError::InvalidPosition)]
    pub sale: Box<Account<'info, ReservationSale>>,
    #[account(address = config.payment_mint @ HubError::InvalidMint)]
    pub payment_mint: Box<Account<'info, Mint>>,
    #[account(token::mint = payment_mint, token::authority = buyer)]
    pub buyer_payment: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed, payer = buyer, space = 8 + HubReservation::INIT_SPACE, seeds = [HUB_RESERVATION_SEED, &hub_id.to_le_bytes(), buyer.key().as_ref()], bump)]
    pub reservation: Box<Account<'info, HubReservation>>,
    #[account(init_if_needed, payer = buyer, space = 8 + PaymentReservation::INIT_SPACE, seeds = [PAYMENT_RESERVATION_SEED, buyer_payment.key().as_ref()], bump)]
    pub payment_reservation: Box<Account<'info, PaymentReservation>>,
    pub system_program: Program<'info, System>,
}

pub fn reserve_tokens_handler(
    ctx: Context<ReserveTokens>,
    hub_id: u64,
    amount: u64,
    max_total_cost: u64,
) -> Result<()> {
    let hub = &ctx.accounts.hub;
    require!(hub.status == HubStatus::Reserving, HubError::InvalidStatus);
    let now = Clock::get()?.unix_timestamp;
    require!(now < hub.sale_deadline, HubError::SaleExpired);
    require!(amount > 0, HubError::InvalidAmount);
    let reserved = ctx
        .accounts
        .sale
        .reserved_tokens
        .checked_add(amount)
        .ok_or(HubError::Overflow)?;
    let allocated = hub
        .tokens_sold
        .checked_add(reserved)
        .ok_or(HubError::Overflow)?;
    require!(allocated <= hub.token_supply, HubError::InvalidAmount);
    let cost = hub
        .token_price
        .checked_mul(amount)
        .ok_or(HubError::Overflow)?;
    require!(cost <= max_total_cost, HubError::CostTooHigh);

    let payment_account = ctx.accounts.buyer_payment.key();
    let reservation = &mut ctx.accounts.reservation;
    if reservation.buyer == Pubkey::default() {
        reservation.hub = hub.key();
        reservation.buyer = ctx.accounts.buyer.key();
        reservation.payment_account = payment_account;
        reservation.bump = ctx.bumps.reservation;
    }
    require!(
        reservation.hub == hub.key()
            && reservation.buyer == ctx.accounts.buyer.key()
            && reservation.payment_account == payment_account,
        HubError::ReservationAccountMismatch
    );
    let payment_reservation = &mut ctx.accounts.payment_reservation;
    if payment_reservation.payment_account == Pubkey::default() {
        payment_reservation.payment_account = payment_account;
        payment_reservation.bump = ctx.bumps.payment_reservation;
    }
    require!(
        payment_reservation.payment_account == payment_account,
        HubError::ReservationAccountMismatch
    );
    let promised = payment_reservation
        .amount
        .checked_add(cost)
        .ok_or(HubError::Overflow)?;
    // This is a balance check, not custody. Users can still spend the tokens;
    // a later paid claim must check and collect the money atomically.
    require!(
        ctx.accounts.buyer_payment.amount >= promised,
        HubError::ReservationBalanceTooLow
    );
    reservation.amount = reservation
        .amount
        .checked_add(amount)
        .ok_or(HubError::Overflow)?;
    reservation.quoted_payment = reservation
        .quoted_payment
        .checked_add(cost)
        .ok_or(HubError::Overflow)?;
    payment_reservation.amount = promised;
    ctx.accounts.sale.reserved_tokens = reserved;
    if allocated == hub.token_supply {
        // Starting the window inside the last reservation avoids a backend
        // race and gives buyers three full days even near the sale deadline.
        ctx.accounts.sale.claim_started_at = now;
        ctx.accounts.sale.claim_deadline = now
            .checked_add(CLAIM_WINDOW_SECONDS)
            .ok_or(HubError::Overflow)?;
        ctx.accounts.hub.status = HubStatus::Claiming;
        emit!(ClaimWindowOpened {
            hub_id,
            deadline: ctx.accounts.sale.claim_deadline
        });
    }
    emit!(TokensReserved {
        hub_id,
        buyer: ctx.accounts.buyer.key(),
        amount,
        quoted_payment: cost
    });
    Ok(())
}

#[event]
pub struct ReservationsOpened {
    pub hub_id: u64,
}
#[event]
pub struct TokensReserved {
    pub hub_id: u64,
    pub buyer: Pubkey,
    pub amount: u64,
    pub quoted_payment: u64,
}
#[event]
pub struct ClaimWindowOpened {
    pub hub_id: u64,
    pub deadline: i64,
}

/// Leaving an unpaid reservation does not refund anything: the payment never
/// left the wallet. No role gate may prevent a buyer releasing an old promise.
#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct CancelReservation<'info> {
    pub buyer: Signer<'info>,
    #[account(seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(mut, seeds = [RESERVATION_SALE_SEED, &hub_id.to_le_bytes()], bump = sale.bump, has_one = hub @ HubError::InvalidPosition)]
    pub sale: Box<Account<'info, ReservationSale>>,
    #[account(mut, seeds = [HUB_RESERVATION_SEED, &hub_id.to_le_bytes(), buyer.key().as_ref()], bump = reservation.bump, has_one = hub @ HubError::InvalidPosition, has_one = buyer @ HubError::InvalidPosition)]
    pub reservation: Box<Account<'info, HubReservation>>,
    #[account(mut, seeds = [PAYMENT_RESERVATION_SEED, reservation.payment_account.as_ref()], bump = payment_reservation.bump)]
    pub payment_reservation: Box<Account<'info, PaymentReservation>>,
}

pub fn cancel_reservation_handler(ctx: Context<CancelReservation>, hub_id: u64) -> Result<()> {
    // Full reservation commits the claiming round. It cannot be reopened by a
    // last-minute cancellation; expired promises use permissionless cleanup.
    require!(
        ctx.accounts.hub.status == HubStatus::Reserving,
        HubError::InvalidStatus
    );
    let amount = ctx.accounts.reservation.amount;
    release_position(
        &mut ctx.accounts.sale,
        &mut ctx.accounts.reservation,
        &mut ctx.accounts.payment_reservation,
    )?;
    emit!(ReservationCancelled {
        hub_id,
        buyer: ctx.accounts.buyer.key(),
        amount
    });
    Ok(())
}

pub(super) fn release_position(
    sale: &mut ReservationSale,
    reservation: &mut HubReservation,
    payment: &mut PaymentReservation,
) -> Result<()> {
    require!(reservation.amount > 0, HubError::EmptyPosition);
    require!(
        payment.payment_account == reservation.payment_account,
        HubError::ReservationAccountMismatch
    );
    sale.reserved_tokens = sale
        .reserved_tokens
        .checked_sub(reservation.amount)
        .ok_or(HubError::Overflow)?;
    payment.amount = payment
        .amount
        .checked_sub(reservation.quoted_payment)
        .ok_or(HubError::Overflow)?;
    reservation.amount = 0;
    reservation.quoted_payment = 0;
    Ok(())
}

#[event]
pub struct ReservationCancelled {
    pub hub_id: u64,
    pub buyer: Pubkey,
    pub amount: u64,
}

/// Anyone can clear an expired unpaid promise. The recorded buyer and payment
/// account determine which reservation is released; the caller receives nothing.
#[derive(Accounts)]
#[instruction(hub_id: u64, buyer: Pubkey)]
pub struct ReleaseReservation<'info> {
    pub cranker: Signer<'info>,
    #[account(seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(mut, seeds = [RESERVATION_SALE_SEED, &hub_id.to_le_bytes()], bump = sale.bump, has_one = hub @ HubError::InvalidPosition)]
    pub sale: Box<Account<'info, ReservationSale>>,
    #[account(mut, seeds = [HUB_RESERVATION_SEED, &hub_id.to_le_bytes(), buyer.as_ref()], bump = reservation.bump, has_one = hub @ HubError::InvalidPosition, constraint = reservation.buyer == buyer @ HubError::InvalidPosition)]
    pub reservation: Box<Account<'info, HubReservation>>,
    #[account(mut, seeds = [PAYMENT_RESERVATION_SEED, reservation.payment_account.as_ref()], bump = payment_reservation.bump)]
    pub payment_reservation: Box<Account<'info, PaymentReservation>>,
}

pub fn release_reservation_handler(
    ctx: Context<ReleaseReservation>,
    hub_id: u64,
    buyer: Pubkey,
) -> Result<()> {
    require_expired(&ctx.accounts.hub, &ctx.accounts.sale)?;
    let amount = ctx.accounts.reservation.amount;
    release_position(
        &mut ctx.accounts.sale,
        &mut ctx.accounts.reservation,
        &mut ctx.accounts.payment_reservation,
    )?;
    emit!(ReservationReleased {
        hub_id,
        buyer,
        amount
    });
    Ok(())
}

fn require_expired(hub: &Hub, sale: &ReservationSale) -> Result<()> {
    let deadline = match hub.status {
        HubStatus::Reserving => hub.sale_deadline,
        HubStatus::Claiming => sale.claim_deadline,
        HubStatus::Failed => return Ok(()),
        _ => return err!(HubError::InvalidStatus),
    };
    require!(
        Clock::get()?.unix_timestamp >= deadline,
        HubError::ReservationStillActive
    );
    Ok(())
}

#[event]
pub struct ReservationReleased {
    pub hub_id: u64,
    pub buyer: Pubkey,
    pub amount: u64,
}

#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct FinalizeReservations<'info> {
    pub cranker: Signer<'info>,
    #[account(mut, seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(seeds = [RESERVATION_SALE_SEED, &hub_id.to_le_bytes()], bump = sale.bump, has_one = hub @ HubError::InvalidPosition)]
    pub sale: Box<Account<'info, ReservationSale>>,
}

pub fn finalize_reservations_handler(
    ctx: Context<FinalizeReservations>,
    hub_id: u64,
) -> Result<()> {
    let hub = &mut ctx.accounts.hub;
    require!(
        matches!(hub.status, HubStatus::Reserving | HubStatus::Claiming),
        HubError::InvalidStatus
    );
    require_expired(hub, &ctx.accounts.sale)?;
    // Only an entirely unpaid campaign can use the existing bond-return rule.
    // Partially paid campaigns reopen their unpaid allocations. They must not
    // fail through this path or return the operator's bond while buyers hold tokens.
    require!(
        hub.total_paid == 0 && hub.tokens_sold == 0 && hub.tokens_claimed == 0,
        HubError::PurchasesOutstanding
    );
    hub.status = HubStatus::Failed;
    emit!(ReservationsFailed { hub_id });
    Ok(())
}

#[event]
pub struct ReservationsFailed {
    pub hub_id: u64,
}

/// Unpaid records must be cleared before a new round starts, so an old promise
/// cannot become claimable again under the new round's deadline.
#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct ReopenReservations<'info> {
    pub cranker: Signer<'info>,
    #[account(mut, seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(mut, seeds = [RESERVATION_SALE_SEED, &hub_id.to_le_bytes()], bump = sale.bump, has_one = hub @ HubError::InvalidPosition)]
    pub sale: Box<Account<'info, ReservationSale>>,
}

pub fn reopen_reservations_handler(ctx: Context<ReopenReservations>, hub_id: u64) -> Result<()> {
    let hub = &mut ctx.accounts.hub;
    require!(
        matches!(hub.status, HubStatus::Reserving | HubStatus::Claiming)
            && hub.tokens_sold > 0
            && hub.tokens_sold < hub.token_supply
            && hub.tokens_claimed == hub.tokens_sold,
        HubError::InvalidStatus
    );
    require_expired(hub, &ctx.accounts.sale)?;
    require!(
        ctx.accounts.sale.reserved_tokens == 0,
        HubError::UnpaidReservationsOutstanding
    );
    hub.sale_deadline = Clock::get()?
        .unix_timestamp
        .checked_add(hub.sale_duration)
        .ok_or(HubError::Overflow)?;
    hub.status = HubStatus::Reserving;
    ctx.accounts.sale.claim_started_at = 0;
    ctx.accounts.sale.claim_deadline = 0;
    emit!(ReservationsReopened {
        hub_id,
        remaining_tokens: hub
            .token_supply
            .checked_sub(hub.tokens_sold)
            .ok_or(HubError::Overflow)?,
        sale_deadline: hub.sale_deadline,
    });
    Ok(())
}

#[event]
pub struct ReservationsReopened {
    pub hub_id: u64,
    pub remaining_tokens: u64,
    pub sale_deadline: i64,
}
