use anchor_lang::prelude::*;
use anchor_spl::token_2022::{freeze_account, thaw_account, FreezeAccount, ThawAccount, Token2022};
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::constants::{
    CONFIG_SEED, LISTING_SEED, LISTING_VAULT_SEED, MINT_AUTH_SEED, POSITION_SEED, PROPERTY_SEED,
    PROPERTY_VAULT_SEED, SHARE_MINT_SEED, SHARE_SEED, VAULT_SEED,
};
use crate::error::MarketplaceError;
use crate::state::{Config, InvestorPosition, Listing, ListingStatus, PropertyAsset, ShareHolding};
use crate::vault::release_from_vault;

/// Take everything back out of a listing that expired before selling out:
/// the shares return to the property vault and the full payment, fee and tax
/// included, comes back from the listing vault. The first withdrawal moves
/// the listing to `Expired`. Both accounts close, since a dead listing can't
/// be bought into again. Deliberately not role-gated: exits never are.
#[derive(Accounts)]
#[instruction(listing_id: u64)]
pub struct WithdrawExpired<'info> {
    pub investor: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    /// CHECK: the sponsor wallet that fronted the accounts' rent; gets it
    /// back as they close.
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

    #[account(
        mut,
        close = rent_collector,
        seeds = [POSITION_SEED, &listing_id.to_le_bytes(), investor.key().as_ref()],
        bump = position.bump,
    )]
    pub position: Box<Account<'info, InvestorPosition>>,

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

pub fn withdraw_expired_handler(ctx: Context<WithdrawExpired>, listing_id: u64) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    match ctx.accounts.listing.status {
        // The first withdrawal proves the expiry and flips the status, which
        // also opens the cancelled-position crank.
        ListingStatus::Listed => {
            require!(
                now >= ctx.accounts.listing.listing_expiry,
                MarketplaceError::ListingNotExpired
            );
            ctx.accounts.listing.status = ListingStatus::Expired;
        }
        ListingStatus::Expired => {}
        _ => return err!(MarketplaceError::ListingNotActive),
    }
    let investor = ctx.accounts.investor.key();
    let (amount, refund, payment_mint) = settle_dead_listing_exit(ctx, listing_id)?;

    emit!(ExpiredSharesWithdrawn {
        listing_id,
        investor,
        amount,
        payment_mint,
        refunded: refund,
    });
    Ok(())
}

/// The timeout exit for a sale that sold out but whose legal process never
/// settled: once the deadline passes, investors take their money back the
/// same way they would from an expired listing. The first withdrawal moves
/// the listing to `Refunding`. This is what keeps a successful sale from
/// ever being a trap.
pub fn withdraw_legal_process_expired_handler(
    ctx: Context<WithdrawExpired>,
    listing_id: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    match ctx.accounts.listing.status {
        ListingStatus::SoldOut => {
            require!(
                now >= ctx.accounts.listing.legal_deadline,
                MarketplaceError::LegalProcessNotExpired
            );
            ctx.accounts.listing.status = ListingStatus::Refunding;
        }
        ListingStatus::Refunding => {}
        _ => return err!(MarketplaceError::ListingNotActive),
    }
    let investor = ctx.accounts.investor.key();
    let (amount, refund, payment_mint) = settle_dead_listing_exit(ctx, listing_id)?;

    emit!(LegalTimeoutSharesWithdrawn {
        listing_id,
        investor,
        amount,
        payment_mint,
        refunded: refund,
    });
    Ok(())
}

/// The shared exit body: shares back to the property vault, the full payment
/// back to the investor, both per-investor accounts closed, the counters
/// unwound. Callers have already validated and transitioned the status.
fn settle_dead_listing_exit(
    ctx: Context<WithdrawExpired>,
    listing_id: u64,
) -> Result<(u32, u64, Pubkey)> {
    let amount = ctx.accounts.position.share_amount;
    require!(amount > 0, MarketplaceError::NothingToUnreserve);
    // Closing the position would orphan an unclaimed reservation: the
    // position is the only key to it. The release crank clears it first.
    require!(
        ctx.accounts.position.reserved_share_amount == 0,
        MarketplaceError::ReservationOutstanding
    );
    // The ledger must agree with the position before it closes on the
    // position's number, and no locked share may leave.
    require!(
        ctx.accounts.holding.amount == amount,
        MarketplaceError::LedgerMismatch
    );
    require!(
        ctx.accounts.holding.locked_amount == 0,
        MarketplaceError::SharesLocked
    );

    let refund = ctx
        .accounts
        .position
        .paid_funds
        .checked_add(ctx.accounts.position.paid_fee)
        .and_then(|r| r.checked_add(ctx.accounts.position.paid_tax))
        .ok_or(MarketplaceError::Overflow)?;

    // Shares back to the property vault through the usual airlock.
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
    // The position closes with this exit.
    listing.position_count = listing
        .position_count
        .checked_sub(1)
        .ok_or(MarketplaceError::Overflow)?;

    Ok((amount, refund, ctx.accounts.position.payment_mint))
}

/// Give the developer their XCAV deposit back once the listing is dead with
/// no shares in investor hands: abandoned before the assets ever existed, or
/// expired with nothing sold (or everything withdrawn). Developer-only, and
/// deliberately not role-gated: exits never are.
#[derive(Accounts)]
#[instruction(listing_id: u64)]
pub struct WithdrawDepositUnsold<'info> {
    pub developer: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [LISTING_SEED, &listing_id.to_le_bytes()],
        bump = listing.bump,
        constraint = listing.developer == developer.key() @ MarketplaceError::NotListingDeveloper,
    )]
    pub listing: Box<Account<'info, Listing>>,

    /// The XCAV mint (for `transfer_checked`).
    #[account(address = config.xcav_mint @ MarketplaceError::InvalidMint)]
    pub xcav_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The developer's XCAV account the deposit is returned to.
    #[account(
        mut,
        token::mint = config.xcav_mint,
        token::authority = developer,
    )]
    pub developer_token: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The protocol's XCAV vault.
    #[account(
        mut,
        seeds = [VAULT_SEED],
        bump,
        token::mint = config.xcav_mint,
        token::authority = config,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn withdraw_deposit_unsold_handler(
    ctx: Context<WithdrawDepositUnsold>,
    listing_id: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    match ctx.accounts.listing.status {
        // Never opened for sale: the developer can abandon it right away.
        ListingStatus::PendingAssets => {}
        ListingStatus::Listed | ListingStatus::Expired => {
            require!(
                now >= ctx.accounts.listing.listing_expiry,
                MarketplaceError::ListingNotExpired
            );
        }
        // These already proved their own conditions (Cancelled arrives with
        // the documents-rejection flow); once every investor has withdrawn,
        // the deposit follows.
        ListingStatus::Refunding | ListingStatus::Cancelled => {}
        _ => return err!(MarketplaceError::ListingNotActive),
    }
    require!(
        ctx.accounts.listing.sold_share_amount == 0,
        MarketplaceError::SharesOutstanding
    );
    let deposit = ctx.accounts.listing.deposit;
    require!(deposit > 0, MarketplaceError::DepositAlreadyWithdrawn);

    release_from_vault(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.xcav_mint.to_account_info(),
        &ctx.accounts.developer_token.to_account_info(),
        &ctx.accounts.config.to_account_info(),
        ctx.accounts.config.bump,
        deposit,
        ctx.accounts.xcav_mint.decimals,
    )?;

    let listing = &mut ctx.accounts.listing;
    listing.deposit = 0;
    if !matches!(
        listing.status,
        ListingStatus::Refunding | ListingStatus::Cancelled
    ) {
        listing.status = ListingStatus::Expired;
    }

    emit!(ListingDepositWithdrawn {
        listing_id,
        developer: ctx.accounts.developer.key(),
        deposit,
    });
    Ok(())
}

#[event]
pub struct ExpiredSharesWithdrawn {
    pub listing_id: u64,
    pub investor: Pubkey,
    pub amount: u32,
    pub payment_mint: Pubkey,
    pub refunded: u64,
}

#[event]
pub struct LegalTimeoutSharesWithdrawn {
    pub listing_id: u64,
    pub investor: Pubkey,
    pub amount: u32,
    pub payment_mint: Pubkey,
    pub refunded: u64,
}

#[event]
pub struct ListingDepositWithdrawn {
    pub listing_id: u64,
    pub developer: Pubkey,
    pub deposit: u64,
}
