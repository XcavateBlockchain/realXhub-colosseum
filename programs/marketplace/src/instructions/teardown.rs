use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::{
    extension::StateWithExtensions, state::Account as TokenAccountState, state::Mint as MintState,
};
use anchor_spl::token_2022::{burn, close_account, Burn, CloseAccount, Token2022};
use anchor_spl::token_interface::{
    close_account as close_payment_account, transfer_checked, CloseAccount as ClosePaymentAccount,
    TokenInterface, TransferChecked,
};

use crate::constants::{
    CONFIG_SEED, LISTING_SEED, LISTING_VAULT_SEED, MINT_AUTH_SEED, PROPERTY_SEED,
    PROPERTY_VAULT_SEED,
};
use crate::error::MarketplaceError;
use crate::state::{Config, Listing, ListingStatus, PropertyAsset};

/// Sweep everything a fully wound-down listing leaves behind: burn the share
/// supply sitting in the property vault, close the mint and the vault's share
/// account back to the developer who paid their rent, close the listing
/// vault's payment accounts back to the sponsor, and close the listing and
/// property records themselves. Permissionless; only reachable once every
/// deposit and refund has already been paid out. Leftover balances (donated
/// dust, fees whose settlement the pot outlasted) sweep to the treasury, so
/// nobody can pin the listing open with one base unit; retained fees the SPV
/// lawyer is still owed refuse the sweep until `settle_cancelled_fees` runs.
///
/// The remaining accounts cover every accepted payment mint, in the config's
/// order, as (vault account, mint, treasury account) triples; the full list
/// is what stops a teardown from quietly leaving a funded account behind.
/// Vault accounts that never existed just skip.
#[derive(Accounts)]
#[instruction(listing_id: u64)]
pub struct CloseDeadListing<'info> {
    pub cranker: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    /// CHECK: the sponsor wallet; receives the payment accounts' rent.
    #[account(mut, address = config.rent_collector @ MarketplaceError::NotRentCollector)]
    pub rent_collector: UncheckedAccount<'info>,

    /// CHECK: the developer who paid the listing-side rent; receives it back.
    #[account(mut, address = listing.developer @ MarketplaceError::NotListingDeveloper)]
    pub developer: UncheckedAccount<'info>,

    #[account(
        mut,
        close = developer,
        seeds = [LISTING_SEED, &listing_id.to_le_bytes()],
        bump = listing.bump,
    )]
    pub listing: Box<Account<'info, Listing>>,

    #[account(
        mut,
        close = developer,
        seeds = [PROPERTY_SEED, &listing_id.to_le_bytes()],
        bump = property.bump,
    )]
    pub property: Box<Account<'info, PropertyAsset>>,

    /// CHECK: the share mint; absent when the listing died in `PendingAssets`
    /// before the mint existed.
    #[account(mut, address = property.share_mint @ MarketplaceError::InvalidMint)]
    pub share_mint: Option<UncheckedAccount<'info>>,

    /// CHECK: the share mint's authority PDA; signs the mint close.
    #[account(seeds = [MINT_AUTH_SEED, &listing_id.to_le_bytes()], bump)]
    pub mint_auth: UncheckedAccount<'info>,

    /// CHECK: the property vault authority; signs the burn and account close.
    #[account(seeds = [PROPERTY_VAULT_SEED, &listing_id.to_le_bytes()], bump)]
    pub property_vault: UncheckedAccount<'info>,

    /// CHECK: the vault's share account; validated by the token program at
    /// burn and close. Absent together with the mint.
    #[account(mut)]
    pub vault_share_account: Option<UncheckedAccount<'info>>,

    /// CHECK: the listing vault authority; signs the payment-account closes.
    #[account(seeds = [LISTING_VAULT_SEED, &listing_id.to_le_bytes()], bump)]
    pub listing_vault: UncheckedAccount<'info>,

    pub share_token_program: Program<'info, Token2022>,
    /// The token program owning the remaining payment accounts.
    pub payment_token_program: Interface<'info, TokenInterface>,
}

pub fn close_dead_listing_handler<'info>(
    ctx: Context<'info, CloseDeadListing<'info>>,
    listing_id: u64,
) -> Result<()> {
    let listing = &ctx.accounts.listing;
    require!(
        matches!(
            listing.status,
            ListingStatus::Expired | ListingStatus::Refunding | ListingStatus::Cancelled
        ),
        MarketplaceError::ListingNotActive
    );
    // An unsettled election still has candidacies keyed to this listing;
    // the finalizer must sweep it first.
    require!(
        listing.spv_election.expiry == 0,
        MarketplaceError::VotingStillOngoing
    );
    // Resigning and the close_case crank both need the listing to clear a
    // lawyer's case count. Closing under an engaged lawyer would pin their
    // registry deposit forever.
    require!(
        listing.developer_lawyer.lawyer == Pubkey::default()
            && listing.spv_lawyer.lawyer == Pubkey::default(),
        MarketplaceError::LawyerStillEngaged
    );
    require!(listing.deposit == 0, MarketplaceError::DepositStillHeld);
    require!(
        listing.sold_share_amount == 0 && ctx.accounts.property.holder_count == 0,
        MarketplaceError::SharesOutstanding
    );
    // Cancelled positions carry no shares, so the share guards can't see
    // them; they still need this listing alive to close against.
    require!(
        listing.position_count == 0,
        MarketplaceError::PositionsOutstanding
    );

    let id_bytes = listing_id.to_le_bytes();
    let auth_seeds: &[&[u8]] = &[MINT_AUTH_SEED, &id_bytes, &[ctx.bumps.mint_auth]];
    let vault_seeds: &[&[u8]] = &[PROPERTY_VAULT_SEED, &id_bytes, &[ctx.bumps.property_vault]];
    let listing_vault_seeds: &[&[u8]] =
        &[LISTING_VAULT_SEED, &id_bytes, &[ctx.bumps.listing_vault]];

    // The mint half only exists if the listing ever reached step two.
    if ctx.accounts.property.share_mint != Pubkey::default() {
        let share_mint = ctx
            .accounts
            .share_mint
            .as_ref()
            .ok_or(MarketplaceError::InvalidMint)?;
        let vault_share_account = ctx
            .accounts
            .vault_share_account
            .as_ref()
            .ok_or(MarketplaceError::InvalidMint)?;
        // Pin the canonical vault account: a decoy with the right owner and
        // mint would skip the burn, and only the mint close would catch it.
        require!(
            vault_share_account.key()
                == anchor_spl::associated_token::get_associated_token_address_with_program_id(
                    &ctx.accounts.property_vault.key(),
                    &share_mint.key(),
                    &ctx.accounts.share_token_program.key(),
                ),
            MarketplaceError::WrongVaultAccount
        );

        // With no holders left, the vault carries the whole supply; burn it
        // so the mint can close.
        let vault_balance = {
            let data = vault_share_account.try_borrow_data()?;
            StateWithExtensions::<TokenAccountState>::unpack(&data)?
                .base
                .amount
        };
        if vault_balance > 0 {
            burn(
                CpiContext::new_with_signer(
                    ctx.accounts.share_token_program.key(),
                    Burn {
                        mint: share_mint.to_account_info(),
                        from: vault_share_account.to_account_info(),
                        authority: ctx.accounts.property_vault.to_account_info(),
                    },
                    &[vault_seeds],
                ),
                vault_balance,
            )?;
        }
        close_account(CpiContext::new_with_signer(
            ctx.accounts.share_token_program.key(),
            CloseAccount {
                account: vault_share_account.to_account_info(),
                destination: ctx.accounts.developer.to_account_info(),
                authority: ctx.accounts.property_vault.to_account_info(),
            },
            &[vault_seeds],
        ))?;
        close_account(CpiContext::new_with_signer(
            ctx.accounts.share_token_program.key(),
            CloseAccount {
                account: share_mint.to_account_info(),
                destination: ctx.accounts.developer.to_account_info(),
                authority: ctx.accounts.mint_auth.to_account_info(),
            },
            &[auth_seeds],
        ))?;
    }

    // The listing vault's payment accounts, one triple per mint the
    // listing ever collected, in `collected` order. Keyed off the
    // listing's own record rather than live config, so rotating a mint
    // out of the accepted list can't strand a balance here.
    let mints: Vec<Pubkey> = ctx
        .accounts
        .listing
        .collected
        .iter()
        .map(|c| c.mint)
        .collect();
    require!(
        ctx.remaining_accounts.len() == mints.len() * 3,
        MarketplaceError::InvalidConfig
    );
    for (expected_mint, triple) in mints.iter().zip(ctx.remaining_accounts.chunks(3)) {
        let (vault_account, mint, treasury_account) = (&triple[0], &triple[1], &triple[2]);
        require!(mint.key == expected_mint, MarketplaceError::InvalidMint);
        // The mint's owner is its token program; the guard vetted it at
        // config time. The CPIs need that program in the transaction, so it
        // must be one of the two the instruction carries.
        let token_program = mint.owner;
        require!(
            *token_program == ctx.accounts.share_token_program.key()
                || *token_program == ctx.accounts.payment_token_program.key(),
            MarketplaceError::InvalidMint
        );
        require!(
            vault_account.key()
                == anchor_spl::associated_token::get_associated_token_address_with_program_id(
                    &ctx.accounts.listing_vault.key(),
                    expected_mint,
                    token_program,
                ),
            MarketplaceError::WrongVaultAccount
        );
        // Never paid with: the account was never created.
        if vault_account.data_is_empty() {
            continue;
        }
        let amount = {
            let data = vault_account.try_borrow_data()?;
            StateWithExtensions::<TokenAccountState>::unpack(&data)?
                .base
                .amount
        };
        if amount > 0 {
            // Money still here is either dust or unsettled fees; the lawyer's
            // share must leave through the settlement first.
            require!(
                ctx.accounts.listing.status != ListingStatus::Cancelled
                    || ctx.accounts.listing.spv_costs_due == 0,
                MarketplaceError::CostsStillDue
            );
            require!(
                treasury_account.key()
                    == anchor_spl::associated_token::get_associated_token_address_with_program_id(
                        &ctx.accounts.config.treasury,
                        expected_mint,
                        token_program,
                    ),
                MarketplaceError::WrongVaultAccount
            );
            let decimals = {
                let data = mint.try_borrow_data()?;
                StateWithExtensions::<MintState>::unpack(&data)?
                    .base
                    .decimals
            };
            transfer_checked(
                CpiContext::new_with_signer(
                    *token_program,
                    TransferChecked {
                        from: vault_account.clone(),
                        mint: mint.clone(),
                        to: treasury_account.clone(),
                        authority: ctx.accounts.listing_vault.to_account_info(),
                    },
                    &[listing_vault_seeds],
                ),
                amount,
                decimals,
            )?;
        }
        close_payment_account(CpiContext::new_with_signer(
            *token_program,
            ClosePaymentAccount {
                account: vault_account.clone(),
                destination: ctx.accounts.rent_collector.to_account_info(),
                authority: ctx.accounts.listing_vault.to_account_info(),
            },
            &[listing_vault_seeds],
        ))?;
    }

    emit!(DeadListingClosed {
        listing_id,
        developer: ctx.accounts.listing.developer,
    });
    Ok(())
}

#[event]
pub struct DeadListingClosed {
    pub listing_id: u64,
    pub developer: Pubkey,
}
