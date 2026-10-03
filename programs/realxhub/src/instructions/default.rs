use anchor_lang::prelude::*;
use anchor_spl::associated_token::{self, AssociatedToken, Create};
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, TransferChecked};

use crate::constants::*;
use crate::error::HubError;
use crate::state::{
    Config, DefaultRedemption, EvidenceStatus, Hub, HubDefault, HubFunding, HubMilestone,
    HubStatus, ReservationClaim,
};

#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct DeclareDefault<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(seeds = [FUNDING_SEED, &hub_id.to_le_bytes()], bump = funding.bump,
        has_one = hub @ HubError::InvalidPosition)]
    pub funding: Box<Account<'info, HubFunding>>,
    /// CHECK: The canonical milestone must be supplied even when no evidence
    /// exists. The handler validates its owner and deserializes any existing
    /// record, so callers cannot omit a successful assessment to claim default.
    #[account(seeds = [MILESTONE_SEED, &hub_id.to_le_bytes()], bump)]
    pub milestone: UncheckedAccount<'info>,
    #[account(address = config.payment_mint @ HubError::InvalidMint)]
    pub payment_mint: Box<Account<'info, Mint>>,
    #[account(address = config.xcav_mint @ HubError::InvalidMint)]
    pub xcav_mint: Box<Account<'info, Mint>>,
    #[account(seeds = [PAYMENT_VAULT_SEED, &hub_id.to_le_bytes()], bump,
        token::mint = payment_mint, token::authority = hub)]
    pub payment_vault: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [BOND_VAULT_SEED, &hub_id.to_le_bytes()], bump,
        token::mint = xcav_mint, token::authority = hub)]
    pub bond_vault: Box<Account<'info, TokenAccount>>,
    #[account(init, payer = cranker, space = 8 + HubDefault::INIT_SPACE,
        seeds = [DEFAULT_SEED, &hub_id.to_le_bytes()], bump)]
    pub settlement: Box<Account<'info, HubDefault>>,
    pub system_program: Program<'info, System>,
}

pub fn declare_default_handler(ctx: Context<DeclareDefault>, hub_id: u64) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let hub = &ctx.accounts.hub;
    let funding = &ctx.accounts.funding;
    let deadline = super::milestone::approval_deadline(hub, funding, now)?;
    require!(now >= deadline, HubError::DefaultStillActive);
    require!(
        !hub.bond_refunded && hub.bond_amount > 0,
        HubError::DefaultNotAllowed
    );

    let milestone = ctx.accounts.milestone.to_account_info();
    if milestone.owner == &anchor_lang::system_program::ID && milestone.data_is_empty() {
        // No evidence was submitted. A donated SOL balance at this address
        // does not change the absence of a program-owned milestone record.
    } else {
        require_keys_eq!(*milestone.owner, crate::ID, HubError::InvalidPosition);
        let data = milestone.try_borrow_data()?;
        let record = HubMilestone::try_deserialize(&mut &data[..])?;
        require!(
            record.hub == hub.key() && record.deadline == deadline,
            HubError::InvalidPosition
        );
        require!(
            !record.second_tranche_released && record.status != EvidenceStatus::Approved,
            HubError::DefaultNotAllowed
        );
    }

    let payment_pool = hub
        .total_paid
        .checked_sub(funding.first_tranche_amount)
        .ok_or(HubError::Overflow)?;
    // Only recorded liabilities enter the pool. Donations are never treated
    // as purchased funding or as a larger operator bond.
    require!(
        ctx.accounts.payment_vault.amount >= payment_pool
            && ctx.accounts.bond_vault.amount >= hub.bond_amount,
        HubError::InsufficientDefaultEscrow
    );
    let settlement = &mut ctx.accounts.settlement;
    settlement.hub = hub.key();
    settlement.token_supply = hub.token_supply;
    settlement.payment_pool = payment_pool;
    settlement.xcav_pool = hub.bond_amount;
    settlement.approval_deadline = deadline;
    settlement.defaulted_at = now;
    settlement.bump = ctx.bumps.settlement;
    ctx.accounts.hub.status = HubStatus::Defaulted;
    emit!(HubDefaultDeclared {
        hub_id,
        payment_pool,
        xcav_pool: settlement.xcav_pool,
        deadline,
        defaulted_at: now,
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct RedeemDefault<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(mut, seeds = [DEFAULT_SEED, &hub_id.to_le_bytes()], bump = settlement.bump,
        has_one = hub @ HubError::InvalidPosition)]
    pub settlement: Box<Account<'info, HubDefault>>,
    #[account(seeds = [RESERVATION_CLAIM_SEED, &hub_id.to_le_bytes(), buyer.key().as_ref()],
        bump = claim.bump, has_one = hub @ HubError::InvalidPosition,
        has_one = buyer @ HubError::InvalidPosition)]
    pub claim: Box<Account<'info, ReservationClaim>>,
    #[account(init_if_needed, payer = buyer, space = 8 + DefaultRedemption::INIT_SPACE,
        seeds = [DEFAULT_REDEMPTION_SEED, &hub_id.to_le_bytes(), buyer.key().as_ref()], bump)]
    pub redemption: Box<Account<'info, DefaultRedemption>>,
    #[account(mut, address = hub.token_mint @ HubError::InvalidMint)]
    pub hub_mint: Box<Account<'info, Mint>>,
    #[account(mut, token::mint = hub_mint, token::authority = buyer)]
    pub buyer_token: Box<Account<'info, TokenAccount>>,
    #[account(address = config.payment_mint @ HubError::InvalidMint)]
    pub payment_mint: Box<Account<'info, Mint>>,
    #[account(address = config.xcav_mint @ HubError::InvalidMint)]
    pub xcav_mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [PAYMENT_VAULT_SEED, &hub_id.to_le_bytes()], bump,
        token::mint = payment_mint, token::authority = hub)]
    pub payment_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [BOND_VAULT_SEED, &hub_id.to_le_bytes()], bump,
        token::mint = xcav_mint, token::authority = hub)]
    pub bond_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: Canonical buyer ATA checked before CPIs and created idempotently.
    #[account(mut)]
    pub buyer_payment: UncheckedAccount<'info>,
    /// CHECK: Canonical buyer ATA checked before CPIs and created idempotently.
    #[account(mut)]
    pub buyer_xcav: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn redeem_default_handler(
    ctx: Context<RedeemDefault>,
    hub_id: u64,
    amount: u64,
    min_payment_out: u64,
    min_xcav_out: u64,
) -> Result<()> {
    let hub = &ctx.accounts.hub;
    let settlement = &ctx.accounts.settlement;
    require!(hub.status == HubStatus::Defaulted, HubError::InvalidStatus);
    require!(amount > 0, HubError::InvalidAmount);
    require!(
        settlement.token_supply == hub.token_supply
            && ctx.accounts.claim.amount > 0
            && ctx.accounts.claim.paid
                == hub
                    .token_price
                    .checked_mul(ctx.accounts.claim.amount)
                    .ok_or(HubError::Overflow)?,
        HubError::InvalidPosition
    );
    require_keys_eq!(
        ctx.accounts.buyer_payment.key(),
        associated_token::get_associated_token_address(
            &ctx.accounts.buyer.key(),
            &ctx.accounts.payment_mint.key()
        ),
        HubError::InvalidPosition
    );
    require_keys_eq!(
        ctx.accounts.buyer_xcav.key(),
        associated_token::get_associated_token_address(
            &ctx.accounts.buyer.key(),
            &ctx.accounts.xcav_mint.key()
        ),
        HubError::InvalidPosition
    );
    let redemption = &mut ctx.accounts.redemption;
    if redemption.hub == Pubkey::default() {
        redemption.hub = hub.key();
        redemption.buyer = ctx.accounts.buyer.key();
        redemption.bump = ctx.bumps.redemption;
    }
    require!(
        redemption.hub == hub.key() && redemption.buyer == ctx.accounts.buyer.key(),
        HubError::InvalidPosition
    );
    let buyer_redeemed = redemption
        .amount
        .checked_add(amount)
        .ok_or(HubError::Overflow)?;
    let tokens_redeemed = settlement
        .tokens_redeemed
        .checked_add(amount)
        .ok_or(HubError::Overflow)?;
    require!(
        buyer_redeemed <= ctx.accounts.claim.amount && tokens_redeemed <= settlement.token_supply,
        HubError::RedemptionLimitExceeded
    );
    require!(
        ctx.accounts.buyer_token.amount >= amount,
        HubError::InsufficientRedemptionTokens
    );
    let last = tokens_redeemed == settlement.token_supply;
    let payment = redemption_payout(
        settlement.payment_pool,
        settlement.token_supply,
        redemption.amount,
        buyer_redeemed,
        settlement.payment_redeemed,
        last,
    )?;
    let xcav = redemption_payout(
        settlement.xcav_pool,
        settlement.token_supply,
        redemption.amount,
        buyer_redeemed,
        settlement.xcav_redeemed,
        last,
    )?;
    require!(
        payment >= min_payment_out && xcav >= min_xcav_out,
        HubError::PayoutBelowMinimum
    );

    // No current role gate on recovery of money committed while eligible.
    // Canonical recipients are created inside this transaction; if either
    // transfer fails, both account creations and the burn also roll back.
    for (recipient, mint) in [
        (
            ctx.accounts.buyer_payment.to_account_info(),
            ctx.accounts.payment_mint.to_account_info(),
        ),
        (
            ctx.accounts.buyer_xcav.to_account_info(),
            ctx.accounts.xcav_mint.to_account_info(),
        ),
    ] {
        associated_token::create_idempotent(CpiContext::new(
            ctx.accounts.associated_token_program.key(),
            Create {
                payer: ctx.accounts.buyer.to_account_info(),
                associated_token: recipient,
                authority: ctx.accounts.buyer.to_account_info(),
                mint,
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            },
        ))?;
    }
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Burn {
                mint: ctx.accounts.hub_mint.to_account_info(),
                from: ctx.accounts.buyer_token.to_account_info(),
                authority: ctx.accounts.buyer.to_account_info(),
            },
        ),
        amount,
    )?;
    let id = hub_id.to_le_bytes();
    let bump = [hub.bump];
    let seeds: &[&[u8]] = &[HUB_SEED, &id, &bump];
    for (vault, mint, recipient, quantity, decimals) in [
        (
            ctx.accounts.payment_vault.to_account_info(),
            ctx.accounts.payment_mint.to_account_info(),
            ctx.accounts.buyer_payment.to_account_info(),
            payment,
            ctx.accounts.payment_mint.decimals,
        ),
        (
            ctx.accounts.bond_vault.to_account_info(),
            ctx.accounts.xcav_mint.to_account_info(),
            ctx.accounts.buyer_xcav.to_account_info(),
            xcav,
            ctx.accounts.xcav_mint.decimals,
        ),
    ] {
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: vault,
                    mint,
                    to: recipient,
                    authority: hub.to_account_info(),
                },
                &[seeds],
            ),
            quantity,
            decimals,
        )?;
    }
    redemption.amount = buyer_redeemed;
    redemption.payment_received = redemption
        .payment_received
        .checked_add(payment)
        .ok_or(HubError::Overflow)?;
    redemption.xcav_received = redemption
        .xcav_received
        .checked_add(xcav)
        .ok_or(HubError::Overflow)?;
    let settlement = &mut ctx.accounts.settlement;
    settlement.tokens_redeemed = tokens_redeemed;
    settlement.payment_redeemed = settlement
        .payment_redeemed
        .checked_add(payment)
        .ok_or(HubError::Overflow)?;
    settlement.xcav_redeemed = settlement
        .xcav_redeemed
        .checked_add(xcav)
        .ok_or(HubError::Overflow)?;
    emit!(DefaultTokensRedeemed {
        hub_id,
        buyer: ctx.accounts.buyer.key(),
        amount,
        payment,
        xcav,
    });
    Ok(())
}

/// Cumulative per-buyer floors prevent splitting a redemption from increasing
/// its proportional payout. The final redeemed token receives the remaining
/// base-unit rounding dust so all recorded liabilities can leave escrow.
fn redemption_payout(
    pool: u64,
    supply: u64,
    previous: u64,
    cumulative: u64,
    paid: u64,
    last: bool,
) -> Result<u64> {
    require!(
        supply > 0 && previous <= cumulative && cumulative <= supply && paid <= pool,
        HubError::InvalidPosition
    );
    let remaining = pool.checked_sub(paid).ok_or(HubError::Overflow)?;
    if last {
        return Ok(remaining);
    }
    let entitlement = |quantity: u64| -> u64 {
        // A u64 times a u64 fits into u128; quotient is at most the u64 pool.
        ((u128::from(pool) * u128::from(quantity)) / u128::from(supply)) as u64
    };
    let amount = entitlement(cumulative)
        .checked_sub(entitlement(previous))
        .ok_or(HubError::Overflow)?;
    require!(amount <= remaining, HubError::InvalidPosition);
    Ok(amount)
}

#[event]
pub struct HubDefaultDeclared {
    pub hub_id: u64,
    pub payment_pool: u64,
    pub xcav_pool: u64,
    pub deadline: i64,
    pub defaulted_at: i64,
}

#[event]
pub struct DefaultTokensRedeemed {
    pub hub_id: u64,
    pub buyer: Pubkey,
    pub amount: u64,
    pub payment: u64,
    pub xcav: u64,
}
