use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::constants::{AGENT_SEED, CONFIG_SEED, VAULT_SEED};
use crate::error::PropertyError;
use crate::state::{AgentLocation, Config, LettingAgent, MAX_AGENT_LOCATIONS};
use crate::vault::{lock_to_vault, release_from_vault};

use xcavate_whitelist::state::{Role, RoleAccount};

/// Register as a letting agent for one location. The first location creates
/// the agent's registry entry and fixes their region; later ones must stay in
/// it. Each location locks its own XCAV deposit. LettingAgent-role only; the
/// location must be registered in the region, which the location account's
/// existence proves. Agents are businesses, so they fund their own rent.
#[derive(Accounts)]
#[instruction(region_id: u16, postcode: Vec<u8>)]
pub struct AddLettingAgent<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    /// The caller's LettingAgent role, owned by the roles program.
    #[account(
        seeds = [
            xcavate_whitelist::ROLE_SEED,
            agent.key().as_ref(),
            &[Role::LettingAgent.seed_byte()],
        ],
        bump = agent_role.bump,
        seeds::program = xcavate_whitelist::ID,
    )]
    pub agent_role: Box<Account<'info, RoleAccount>>,

    /// The location, owned by the regions program. Its seeds carry the
    /// region, so this one account proves both exist.
    #[account(
        seeds = [regions::LOCATION_SEED, &region_id.to_le_bytes(), postcode.as_slice()],
        bump = location.bump,
        seeds::program = regions::ID,
    )]
    pub location: Box<Account<'info, regions::state::Location>>,

    #[account(
        init_if_needed,
        payer = agent,
        space = 8 + LettingAgent::INIT_SPACE,
        seeds = [AGENT_SEED, agent.key().as_ref()],
        bump,
    )]
    pub agent_entry: Box<Account<'info, LettingAgent>>,

    #[account(address = config.xcav_mint @ PropertyError::InvalidMint)]
    pub xcav_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The agent's XCAV account the deposit leaves.
    #[account(
        mut,
        token::mint = config.xcav_mint,
        token::authority = agent,
    )]
    pub agent_token: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [VAULT_SEED],
        bump,
        token::mint = config.xcav_mint,
        token::authority = config,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn add_letting_agent_handler(
    ctx: Context<AddLettingAgent>,
    region_id: u16,
    postcode: Vec<u8>,
    max_deposit: u64,
) -> Result<()> {
    // The regions program already bounds postcodes; checking here too keeps
    // the entry's allocated space safe on our own terms.
    require!(
        postcode.len() <= crate::state::POSTCODE_MAX_LEN,
        PropertyError::PostcodeTooLong
    );
    // The deposit is read from live config; the caller caps what they are
    // willing to pay so an update can't reprice their signed transaction.
    let deposit = ctx.accounts.config.agent_deposit;
    require!(deposit <= max_deposit, PropertyError::DepositTooHigh);

    let entry = &mut ctx.accounts.agent_entry;
    if entry.wallet == Pubkey::default() {
        entry.wallet = ctx.accounts.agent.key();
        entry.region_id = region_id;
        entry.rent_payer = ctx.accounts.agent.key();
        entry.bump = ctx.bumps.agent_entry;
    } else {
        require!(entry.region_id == region_id, PropertyError::WrongRegion);
    }
    require!(
        !entry.locations.iter().any(|l| l.postcode == postcode),
        PropertyError::AlreadyInLocation
    );
    require!(
        entry.locations.len() < MAX_AGENT_LOCATIONS,
        PropertyError::TooManyLocations
    );
    entry.locations.push(AgentLocation {
        postcode: postcode.clone(),
        assigned_count: 0,
        deposit,
    });

    lock_to_vault(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.agent_token.to_account_info(),
        &ctx.accounts.xcav_mint.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.agent.to_account_info(),
        deposit,
        ctx.accounts.xcav_mint.decimals,
    )?;

    emit!(LettingAgentAdded {
        agent: ctx.accounts.agent.key(),
        region_id,
        postcode,
        deposit,
    });
    Ok(())
}

/// Leave a location and take the deposit back, allowed only with no
/// properties assigned there. Leaving the last location closes the registry
/// entry. Deliberately not role-gated: exits never are, so an agent whose
/// role lapsed can still withdraw their stake.
#[derive(Accounts)]
pub struct RemoveLettingAgent<'info> {
    pub agent: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [AGENT_SEED, agent.key().as_ref()],
        bump = agent_entry.bump,
    )]
    pub agent_entry: Box<Account<'info, LettingAgent>>,

    /// CHECK: gets the account's rent back when the last location closes the
    /// entry; pinned to whoever funded it.
    #[account(mut, address = agent_entry.rent_payer)]
    pub rent_receiver: UncheckedAccount<'info>,

    #[account(address = config.xcav_mint @ PropertyError::InvalidMint)]
    pub xcav_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The agent's XCAV account the deposit returns to.
    #[account(
        mut,
        token::mint = config.xcav_mint,
        token::authority = agent,
    )]
    pub agent_token: Box<InterfaceAccount<'info, TokenAccount>>,

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

pub fn remove_letting_agent_handler(
    ctx: Context<RemoveLettingAgent>,
    postcode: Vec<u8>,
) -> Result<()> {
    let entry = &mut ctx.accounts.agent_entry;
    let idx = entry
        .locations
        .iter()
        .position(|l| l.postcode == postcode)
        .ok_or(PropertyError::NotInLocation)?;
    require!(
        entry.locations[idx].assigned_count == 0,
        PropertyError::AgentStillAssigned
    );
    let deposit = entry.locations.remove(idx).deposit;
    let empty = entry.locations.is_empty();

    release_from_vault(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.xcav_mint.to_account_info(),
        &ctx.accounts.agent_token.to_account_info(),
        &ctx.accounts.config.to_account_info(),
        ctx.accounts.config.bump,
        deposit,
        ctx.accounts.xcav_mint.decimals,
    )?;

    if empty {
        ctx.accounts
            .agent_entry
            .close(ctx.accounts.rent_receiver.to_account_info())?;
    }

    emit!(LettingAgentRemoved {
        agent: ctx.accounts.agent.key(),
        postcode,
        deposit,
    });
    Ok(())
}

#[event]
pub struct LettingAgentAdded {
    pub agent: Pubkey,
    pub region_id: u16,
    pub postcode: Vec<u8>,
    pub deposit: u64,
}

#[event]
pub struct LettingAgentRemoved {
    pub agent: Pubkey,
    pub postcode: Vec<u8>,
    pub deposit: u64,
}
