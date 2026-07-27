//! The realXmarket marketplace: developers list fractionalized properties,
//! investors buy shares that are delivered directly at purchase, and once a
//! property sells out the legal process hands the deed to an SPV and settles
//! everyone atomically. Deposits (listing, lawyer) are staked in XCAV; sales
//! are paid in the accepted payment mints.

pub mod constants;
pub mod error;
pub mod instructions;
pub mod mint_guard;
pub mod state;
pub mod vault;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::ConfigParams;

use instructions::*;

declare_id!("B6YRVAmjmhN28smZxNfCnuKc19CamBbAEMXsp5KTfWog");

#[program]
pub mod marketplace {
    use super::*;

    pub fn initialize_config(ctx: Context<InitializeConfig>, params: ConfigParams) -> Result<()> {
        initialize::handler(ctx, params)
    }

    pub fn update_config(ctx: Context<UpdateConfig>, params: ConfigParams) -> Result<()> {
        initialize::update_config_handler(ctx, params)
    }

    pub fn update_authority(ctx: Context<UpdateAuthority>, new_authority: Pubkey) -> Result<()> {
        initialize::update_authority_handler(ctx, new_authority)
    }

    pub fn accept_authority(ctx: Context<AcceptAuthority>) -> Result<()> {
        initialize::accept_authority_handler(ctx)
    }

    pub fn register_lawyer(ctx: Context<RegisterLawyer>, region_id: u16) -> Result<()> {
        lawyers::register_lawyer_handler(ctx, region_id)
    }

    pub fn unregister_lawyer(ctx: Context<UnregisterLawyer>) -> Result<()> {
        lawyers::unregister_lawyer_handler(ctx)
    }

    pub fn list_property(
        ctx: Context<ListProperty>,
        region_id: u16,
        postcode: Vec<u8>,
        share_price: u64,
        share_amount: u32,
        tax_paid_by_developer: bool,
    ) -> Result<()> {
        listing::list_property_handler(
            ctx,
            region_id,
            postcode,
            share_price,
            share_amount,
            tax_paid_by_developer,
        )
    }

    pub fn upgrade_object(
        ctx: Context<UpgradeObject>,
        listing_id: u64,
        new_price: u64,
    ) -> Result<()> {
        listing::upgrade_object_handler(ctx, listing_id, new_price)
    }
}
