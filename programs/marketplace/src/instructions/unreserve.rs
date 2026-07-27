use anchor_lang::prelude::*;
use anchor_spl::token_2022::{freeze_account, thaw_account, FreezeAccount, ThawAccount, Token2022};
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::constants::{
    CONFIG_SEED, LISTING_SEED, LISTING_VAULT_SEED, MINT_AUTH_SEED, POSITION_SEED, PROPERTY_SEED,
    PROPERTY_VAULT_SEED, SHARE_MINT_SEED, SHARE_SEED,
};
use crate::error::MarketplaceError;
use crate::state::{Config, InvestorPosition, Listing, ListingStatus, PropertyAsset, ShareHolding};

/// Return every share of a primary-sale position for a full refund, fee and
/// tax included, any time before the listing sells out. One-way: the position
/// stays open with `cancelled` set, and this investor can never buy into this
/// listing again (the frontend warns before they confirm). Deliberately not
/// role-gated: this is a pure exit, and an investor whose role or compliance
/// was revoked must still be able to get their money back.
#[derive(Accounts)]
#[instruction(listing_id: u64)]
pub struct UnreserveShares<'info> {
    pub investor: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    /// CHECK: the sponsor wallet that fronted the holding's rent; gets it back
    /// as the account closes.
    #[account(mut, address = config.rent_collector @ MarketplaceError::NotRentCollector)]
    pub rent_collector: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [LISTING_SEED, &listing_id.to_le_bytes()],
        bump = listing.bump,
    )]
    pub listing: Box<Account<'info, Listing>>,

    #[account(
        mut,
        seeds = [PROPERTY_SEED, &listing_id.to_le_bytes()],
        bump = property.bump,
    )]
    pub property: Box<Account<'info, PropertyAsset>>,

    /// The position being unreserved. Zeroed and flagged, never closed here:
    /// its continued existence is what bars re-buying.
    #[account(
        mut,
        seeds = [POSITION_SEED, &listing_id.to_le_bytes(), investor.key().as_ref()],
        bump = position.bump,
    )]
    pub position: Box<Account<'info, InvestorPosition>>,

    /// The holding closes with the shares gone; a later secondary-market buy
    /// would recreate it.
    #[account(
        mut,
        close = rent_collector,
        seeds = [SHARE_SEED, &listing_id.to_le_bytes(), investor.key().as_ref()],
        bump = holding.bump,
    )]
    pub holding: Box<Account<'info, ShareHolding>>,

    /// The mint the position was paid in; the refund goes out in the same one.
    #[account(address = position.payment_mint @ MarketplaceError::PaymentMintMismatch)]
    pub payment_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The investor's payment account the refund lands in.
    #[account(
        mut,
        token::mint = payment_mint,
        token::authority = investor,
    )]
    pub investor_payment: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: the listing vault authority; a bare PDA owning the vault's
    /// token accounts.
    #[account(seeds = [LISTING_VAULT_SEED, &listing_id.to_le_bytes()], bump)]
    pub listing_vault: UncheckedAccount<'info>,

    /// The vault's account for the payment mint, funded by the buys.
    #[account(
        mut,
        associated_token::mint = payment_mint,
        associated_token::authority = listing_vault,
        associated_token::token_program = payment_token_program,
    )]
    pub listing_payment_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: the share mint PDA (owned by the Token-2022 program).
    #[account(seeds = [SHARE_MINT_SEED, &listing_id.to_le_bytes()], bump)]
    pub share_mint: UncheckedAccount<'info>,

    /// CHECK: the share mint's authority PDA; signs the lock-state changes.
    #[account(seeds = [MINT_AUTH_SEED, &listing_id.to_le_bytes()], bump)]
    pub mint_auth: UncheckedAccount<'info>,

    /// CHECK: the property vault authority; owner of the undistributed shares.
    #[account(seeds = [PROPERTY_VAULT_SEED, &listing_id.to_le_bytes()], bump)]
    pub property_vault: UncheckedAccount<'info>,

    /// The vault's share account the shares return to.
    #[account(
        mut,
        associated_token::mint = share_mint,
        associated_token::authority = property_vault,
        associated_token::token_program = share_token_program,
    )]
    pub vault_share_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The investor's share account the shares leave.
    #[account(
        mut,
        associated_token::mint = share_mint,
        associated_token::authority = investor,
        associated_token::token_program = share_token_program,
    )]
    pub investor_share_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The payment mint's token program (classic or Token-2022).
    pub payment_token_program: Interface<'info, TokenInterface>,
    /// The share mint's program is always Token-2022.
    pub share_token_program: Program<'info, Token2022>,
}

pub fn unreserve_shares_handler(ctx: Context<UnreserveShares>, listing_id: u64) -> Result<()> {
    // Only while the sale is still open; the last share locks it in and the
    // legal process takes over.
    require!(
        ctx.accounts.listing.status == ListingStatus::Listed,
        MarketplaceError::ListingNotActive
    );
    let amount = ctx.accounts.position.share_amount;
    require!(amount > 0, MarketplaceError::NothingToUnreserve);

    let refund = ctx
        .accounts
        .position
        .paid_funds
        .checked_add(ctx.accounts.position.paid_fee)
        .and_then(|r| r.checked_add(ctx.accounts.position.paid_tax))
        .ok_or(MarketplaceError::Overflow)?;

    // Shares back to the property vault: open the investor's account for the
    // one transfer, then lock it again.
    let id_bytes = listing_id.to_le_bytes();
    let auth_seeds: &[&[u8]] = &[MINT_AUTH_SEED, &id_bytes, &[ctx.bumps.mint_auth]];
    let vault_seeds: &[&[u8]] = &[LISTING_VAULT_SEED, &id_bytes, &[ctx.bumps.listing_vault]];
    thaw_account(CpiContext::new_with_signer(
        ctx.accounts.share_token_program.key(),
        ThawAccount {
            account: ctx.accounts.investor_share_account.to_account_info(),
            mint: ctx.accounts.share_mint.to_account_info(),
            authority: ctx.accounts.mint_auth.to_account_info(),
        },
        &[auth_seeds],
    ))?;
    transfer_checked(
        CpiContext::new(
            ctx.accounts.share_token_program.key(),
            TransferChecked {
                from: ctx.accounts.investor_share_account.to_account_info(),
                mint: ctx.accounts.share_mint.to_account_info(),
                to: ctx.accounts.vault_share_account.to_account_info(),
                authority: ctx.accounts.investor.to_account_info(),
            },
        ),
        amount as u64,
        0,
    )?;
    freeze_account(CpiContext::new_with_signer(
        ctx.accounts.share_token_program.key(),
        FreezeAccount {
            account: ctx.accounts.investor_share_account.to_account_info(),
            mint: ctx.accounts.share_mint.to_account_info(),
            authority: ctx.accounts.mint_auth.to_account_info(),
        },
        &[auth_seeds],
    ))?;

    // The refund, in the mint the position paid with.
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.payment_token_program.key(),
            TransferChecked {
                from: ctx.accounts.listing_payment_account.to_account_info(),
                mint: ctx.accounts.payment_mint.to_account_info(),
                to: ctx.accounts.investor_payment.to_account_info(),
                authority: ctx.accounts.listing_vault.to_account_info(),
            },
            &[vault_seeds],
        ),
        refund,
        ctx.accounts.payment_mint.decimals,
    )?;

    let position = &mut ctx.accounts.position;
    position.share_amount = 0;
    position.paid_funds = 0;
    position.paid_fee = 0;
    position.paid_tax = 0;
    position.cancelled = true;

    ctx.accounts.property.holder_count = ctx
        .accounts
        .property
        .holder_count
        .checked_sub(1)
        .ok_or(MarketplaceError::Overflow)?;
    let listing = &mut ctx.accounts.listing;
    listing.sold_share_amount = listing
        .sold_share_amount
        .checked_sub(amount)
        .ok_or(MarketplaceError::Overflow)?;

    emit!(PropertySharesUnreserved {
        listing_id,
        investor: ctx.accounts.investor.key(),
        amount,
        payment_mint: ctx.accounts.position.payment_mint,
        refunded: refund,
    });
    Ok(())
}

/// Reclaim the rent of a cancelled position once the listing has left
/// `Listed`. Permissionless: cancelled investors have nothing locked and no
/// reason to come back, so a crank sweeps the accounts and the sponsor gets
/// its rent back. Never earlier, because the open position is what bars a
/// cancelled investor from re-buying.
#[derive(Accounts)]
#[instruction(listing_id: u64, investor: Pubkey)]
pub struct CloseCancelledPosition<'info> {
    pub cranker: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    /// CHECK: the sponsor wallet that fronted the position's rent.
    #[account(mut, address = config.rent_collector @ MarketplaceError::NotRentCollector)]
    pub rent_collector: UncheckedAccount<'info>,

    #[account(
        seeds = [LISTING_SEED, &listing_id.to_le_bytes()],
        bump = listing.bump,
    )]
    pub listing: Box<Account<'info, Listing>>,

    #[account(
        mut,
        close = rent_collector,
        seeds = [POSITION_SEED, &listing_id.to_le_bytes(), investor.as_ref()],
        bump = position.bump,
        constraint = position.cancelled @ MarketplaceError::PositionNotCancelled,
    )]
    pub position: Box<Account<'info, InvestorPosition>>,
}

pub fn close_cancelled_position_handler(
    ctx: Context<CloseCancelledPosition>,
    listing_id: u64,
    investor: Pubkey,
) -> Result<()> {
    require!(
        ctx.accounts.listing.status != ListingStatus::Listed,
        MarketplaceError::ListingStillActive
    );

    emit!(CancelledPositionClosed {
        listing_id,
        investor,
    });
    Ok(())
}

#[event]
pub struct PropertySharesUnreserved {
    pub listing_id: u64,
    pub investor: Pubkey,
    pub amount: u32,
    pub payment_mint: Pubkey,
    pub refunded: u64,
}

#[event]
pub struct CancelledPositionClosed {
    pub listing_id: u64,
    pub investor: Pubkey,
}
