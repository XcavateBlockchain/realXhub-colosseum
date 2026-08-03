use anchor_lang::prelude::*;

use crate::constants::{LISTING_SEED, PROPERTY_SEED};
use crate::error::MarketplaceError;
use crate::state::{Listing, ListingStatus, PropertyAsset};

use xcavate_whitelist::state::{Role, RoleAccount};

/// Record that the property's SPV has been incorporated. The company is
/// created off chain, so this is an attestation, signed by the dedicated
/// SpvConfirmation role. Role possession only, on purpose: the flag moves no
/// investor funds, and the role is protocol-operated. Callable only once
/// every share is reserved: the attestation locks the sale in and opens the
/// claim window, which is what lets reserved money move.
#[derive(Accounts)]
#[instruction(listing_id: u64)]
pub struct CreateSpv<'info> {
    pub confirmer: Signer<'info>,

    /// The caller's SpvConfirmation role, owned by the roles program.
    #[account(
        seeds = [
            xcavate_whitelist::ROLE_SEED,
            confirmer.key().as_ref(),
            &[Role::SpvConfirmation.seed_byte()],
        ],
        bump = confirmer_role.bump,
        seeds::program = xcavate_whitelist::ID,
    )]
    pub confirmer_role: Box<Account<'info, RoleAccount>>,

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
}

pub fn create_spv_handler(ctx: Context<CreateSpv>, listing_id: u64) -> Result<()> {
    // Selling out needs claims and claims need the SPV, so no later status
    // can still be missing the attestation.
    require!(
        ctx.accounts.listing.status == ListingStatus::Listed,
        MarketplaceError::ListingNotActive
    );
    require!(
        ctx.accounts.listing.reserved_share_amount + ctx.accounts.listing.sold_share_amount
            == ctx.accounts.listing.listed_share_amount,
        MarketplaceError::NotFullyReserved
    );
    require!(
        !ctx.accounts.property.spv_created,
        MarketplaceError::SpvAlreadyCreated
    );

    ctx.accounts.property.spv_created = true;
    // The SPV existing is what lets reserved money move: the claim window
    // opens now and direct purchases take over when it ends.
    let listing = &mut ctx.accounts.listing;
    listing.claim_deadline = Clock::get()?
        .unix_timestamp
        .checked_add(listing.claiming_time)
        .ok_or(MarketplaceError::Overflow)?;

    emit!(SpvCreated {
        listing_id,
        confirmer: ctx.accounts.confirmer.key(),
    });
    Ok(())
}

#[event]
pub struct SpvCreated {
    pub listing_id: u64,
    pub confirmer: Pubkey,
}
