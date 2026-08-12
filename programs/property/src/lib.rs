//! The realXmarket property program: letting agents manage rented-out
//! properties, rental income is distributed to shareholders through a
//! per-mint accumulator, and holders govern the property by share-weighted
//! vote. Agent deposits are staked in XCAV; income arrives in the accepted
//! payment mints.

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

declare_id!("8f4NHc1wGBM1BAufDFd9dNechLW8pxmStSfxfuJfDzob");

#[program]
pub mod property {
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

    pub fn add_letting_agent(
        ctx: Context<AddLettingAgent>,
        region_id: u16,
        postcode: Vec<u8>,
        max_deposit: u64,
    ) -> Result<()> {
        agents::add_letting_agent_handler(ctx, region_id, postcode, max_deposit)
    }

    pub fn remove_letting_agent(
        ctx: Context<RemoveLettingAgent>,
        postcode: Vec<u8>,
    ) -> Result<()> {
        agents::remove_letting_agent_handler(ctx, postcode)
    }
}
