use anchor_lang::prelude::*;
use anchor_spl::associated_token::{self, AssociatedToken, Create};
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use xcavate_whitelist::state::{Role, RoleAccount};

use crate::constants::*;
use crate::error::HubError;
use crate::state::{
    Config, Hub, HubFunding, HubReservation, HubStatus, PaymentReservation, ReservationClaim,
    ReservationSale,
};

use super::funding::release_first_tranche;
use super::reservation::release_position;

#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct ClaimReservedTokens<'info> {
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
    #[account(mut, seeds = [HUB_RESERVATION_SEED, &hub_id.to_le_bytes(), buyer.key().as_ref()], bump = reservation.bump,
        has_one = hub @ HubError::InvalidPosition, has_one = buyer @ HubError::InvalidPosition)]
    pub reservation: Box<Account<'info, HubReservation>>,
    #[account(mut, seeds = [PAYMENT_RESERVATION_SEED, reservation.payment_account.as_ref()], bump = payment_reservation.bump)]
    pub payment_reservation: Box<Account<'info, PaymentReservation>>,
    #[account(address = config.payment_mint @ HubError::InvalidMint)]
    pub payment_mint: Box<Account<'info, Mint>>,
    #[account(mut, address = reservation.payment_account @ HubError::ReservationAccountMismatch,
        token::mint = payment_mint, token::authority = buyer)]
    pub buyer_payment: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [PAYMENT_VAULT_SEED, &hub_id.to_le_bytes()], bump, token::mint = payment_mint, token::authority = hub)]
    pub payment_vault: Box<Account<'info, TokenAccount>>,
    #[account(address = hub.token_mint @ HubError::InvalidMint)]
    pub hub_mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [TOKEN_VAULT_SEED, &hub_id.to_le_bytes()], bump, token::mint = hub_mint, token::authority = hub)]
    pub token_vault: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed, payer = buyer, associated_token::mint = hub_mint, associated_token::authority = buyer)]
    pub buyer_token: Box<Account<'info, TokenAccount>>,
    #[account(constraint = records.buyer.key() == buyer.key() && records.hub.key() == hub.key()
        @ HubError::InvalidPosition)]
    pub records: ClaimRecords<'info>,
    /// CHECK: Fixed to the recorded operator, never the caller's recipient.
    #[account(address = hub.operator @ HubError::NotOperator)]
    pub operator: UncheckedAccount<'info>,
    /// CHECK: The handler checks the canonical operator ATA before any transfer,
    /// then creates/validates it through the associated-token program.
    #[account(mut)]
    pub operator_payment: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// Separate account initialization keeps each validation frame within Solana's
/// stack limit. The parent binds this buyer to the payment and token recipient.
#[derive(Accounts)]
pub struct ClaimRecords<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    pub hub: Box<Account<'info, Hub>>,
    #[account(init_if_needed, payer = buyer, space = 8 + ReservationClaim::INIT_SPACE,
        seeds = [RESERVATION_CLAIM_SEED, &hub.hub_id.to_le_bytes(), buyer.key().as_ref()], bump)]
    pub claim: Box<Account<'info, ReservationClaim>>,
    #[account(init_if_needed, payer = buyer, space = 8 + HubFunding::INIT_SPACE,
        seeds = [FUNDING_SEED, &hub.hub_id.to_le_bytes()], bump)]
    pub funding: Box<Account<'info, HubFunding>>,
    pub system_program: Program<'info, System>,
}

pub fn claim_reserved_tokens_handler(
    ctx: Context<ClaimReservedTokens>,
    hub_id: u64,
    max_total_cost: u64,
) -> Result<()> {
    let hub = &ctx.accounts.hub;
    require_keys_eq!(
        ctx.accounts.operator_payment.key(),
        associated_token::get_associated_token_address(
            &hub.operator,
            &ctx.accounts.payment_mint.key()
        ),
        HubError::InvalidPosition
    );
    require!(hub.status == HubStatus::Claiming, HubError::InvalidStatus);
    let now = Clock::get()?.unix_timestamp;
    require!(
        now < ctx.accounts.sale.claim_deadline,
        HubError::SaleExpired
    );
    let amount = ctx.accounts.reservation.amount;
    let cost = ctx.accounts.reservation.quoted_payment;
    require!(amount > 0, HubError::EmptyPosition);
    require!(cost <= max_total_cost, HubError::CostTooHigh);
    require!(
        cost == hub
            .token_price
            .checked_mul(amount)
            .ok_or(HubError::Overflow)?,
        HubError::InvalidPosition
    );
    let tokens_sold = hub
        .tokens_sold
        .checked_add(amount)
        .ok_or(HubError::Overflow)?;
    let tokens_claimed = hub
        .tokens_claimed
        .checked_add(amount)
        .ok_or(HubError::Overflow)?;
    let total_paid = hub.total_paid.checked_add(cost).ok_or(HubError::Overflow)?;
    require!(
        tokens_sold <= hub.token_supply && tokens_claimed == tokens_sold,
        HubError::InvalidAmount
    );

    let claim = &mut ctx.accounts.records.claim;
    if claim.hub == Pubkey::default() {
        claim.hub = hub.key();
        claim.buyer = ctx.accounts.buyer.key();
        claim.bump = ctx.bumps.records.claim;
    }
    require!(
        claim.hub == hub.key() && claim.buyer == ctx.accounts.buyer.key(),
        HubError::InvalidPosition
    );
    let claimed_amount = claim.amount.checked_add(amount).ok_or(HubError::Overflow)?;
    let claimed_paid = claim.paid.checked_add(cost).ok_or(HubError::Overflow)?;

    let funding = &mut ctx.accounts.records.funding;
    if funding.hub == Pubkey::default() {
        funding.hub = hub.key();
        funding.bump = ctx.bumps.records.funding;
    }
    require!(funding.hub == hub.key(), HubError::InvalidPosition);

    // The reservation was only a promise. Collect the payment now, then deliver
    // the entire reserved allocation in the same transaction. A failed delivery
    // rolls the payment back along with account initialization and all counters.
    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.buyer_payment.to_account_info(),
                mint: ctx.accounts.payment_mint.to_account_info(),
                to: ctx.accounts.payment_vault.to_account_info(),
                authority: ctx.accounts.buyer.to_account_info(),
            },
        ),
        cost,
        ctx.accounts.payment_mint.decimals,
    )?;
    let id_bytes = hub_id.to_le_bytes();
    let bump = [hub.bump];
    let seeds: &[&[u8]] = &[HUB_SEED, &id_bytes, &bump];
    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.token_vault.to_account_info(),
                mint: ctx.accounts.hub_mint.to_account_info(),
                to: ctx.accounts.buyer_token.to_account_info(),
                authority: hub.to_account_info(),
            },
            &[seeds],
        ),
        amount,
        ctx.accounts.hub_mint.decimals,
    )?;
    release_position(
        &mut ctx.accounts.sale,
        &mut ctx.accounts.reservation,
        &mut ctx.accounts.payment_reservation,
    )?;
    let claim = &mut ctx.accounts.records.claim;
    claim.amount = claimed_amount;
    claim.paid = claimed_paid;
    claim.last_claimed_at = now;

    let hub = &mut ctx.accounts.hub;
    hub.tokens_sold = tokens_sold;
    hub.tokens_claimed = tokens_claimed;
    hub.total_paid = total_paid;
    emit!(ReservationTokensClaimed {
        hub_id,
        buyer: ctx.accounts.buyer.key(),
        amount,
        cost,
    });
    if tokens_claimed == hub.token_supply {
        // Initialize this recipient only on the final claim. Keeping the ATA
        // CPI here also keeps account validation below Solana's stack limit.
        associated_token::create_idempotent(CpiContext::new(
            ctx.accounts.associated_token_program.key(),
            Create {
                payer: ctx.accounts.buyer.to_account_info(),
                associated_token: ctx.accounts.operator_payment.to_account_info(),
                authority: ctx.accounts.operator.to_account_info(),
                mint: ctx.accounts.payment_mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            },
        ))?;
        hub.status = HubStatus::Funded;
        release_first_tranche(
            hub,
            &mut ctx.accounts.records.funding,
            ctx.bumps.records.funding,
            CpiContext::new(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.payment_vault.to_account_info(),
                    mint: ctx.accounts.payment_mint.to_account_info(),
                    to: ctx.accounts.operator_payment.to_account_info(),
                    authority: hub.to_account_info(),
                },
            ),
            ctx.accounts.payment_mint.decimals,
            now,
        )?;
        emit!(ReservationFundingCompleted { hub_id, total_paid });
    }
    Ok(())
}

#[event]
pub struct ReservationTokensClaimed {
    pub hub_id: u64,
    pub buyer: Pubkey,
    pub amount: u64,
    pub cost: u64,
}

#[event]
pub struct ReservationFundingCompleted {
    pub hub_id: u64,
    pub total_paid: u64,
}
