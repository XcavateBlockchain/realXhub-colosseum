use anchor_lang::prelude::*;

use crate::constants::{AGENT_SEED, CONFIG_SEED, LETTING_SEED, RESIGNATION_SEED};
use crate::error::PropertyError;
use crate::state::{Config, LettingAgent, PropertyLetting, ResignationNotice};

use marketplace::state::PropertyAsset;

/// An assigned agent gives notice that they are stepping down from a
/// property. The seat only opens once the notice period has run, so the
/// property is never without a manager overnight. Deliberately not
/// role-gated: exits never are.
#[derive(Accounts)]
#[instruction(asset_id: u64)]
pub struct Resign<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        seeds = [LETTING_SEED, &asset_id.to_le_bytes()],
        bump = letting.bump,
        constraint = letting.agent == agent.key() @ PropertyError::NotAssignedAgent,
    )]
    pub letting: Box<Account<'info, PropertyLetting>>,

    /// The notice, one per property; `init` rejects resigning twice.
    #[account(
        init,
        payer = agent,
        space = 8 + ResignationNotice::INIT_SPACE,
        seeds = [RESIGNATION_SEED, &asset_id.to_le_bytes()],
        bump,
    )]
    pub notice: Box<Account<'info, ResignationNotice>>,

    pub system_program: Program<'info, System>,
}

pub fn resign_handler(ctx: Context<Resign>, asset_id: u64) -> Result<()> {
    let notice = &mut ctx.accounts.notice;
    notice.asset_id = asset_id;
    notice.agent = ctx.accounts.agent.key();
    notice.due_ts = Clock::get()?
        .unix_timestamp
        .checked_add(ctx.accounts.config.agent_notice_period)
        .ok_or(PropertyError::Overflow)?;
    notice.rent_payer = ctx.accounts.agent.key();
    notice.bump = ctx.bumps.notice;

    emit!(AgentResignationInitiated {
        asset_id,
        agent: notice.agent,
        due_ts: notice.due_ts,
    });
    Ok(())
}

/// Complete a resignation once its notice period has run: the seat opens,
/// the agent's assignment count drops, and the notice closes. Permissionless
/// crank; there are no queues, the notice carries its own due time.
#[derive(Accounts)]
#[instruction(asset_id: u64)]
pub struct FinalizeResignation<'info> {
    pub cranker: Signer<'info>,

    /// CHECK: the wallet that fronted the notice's rent; gets it back as the
    /// notice closes.
    #[account(mut, address = notice.rent_payer @ PropertyError::WrongRentPayer)]
    pub rent_payer: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [LETTING_SEED, &asset_id.to_le_bytes()],
        bump = letting.bump,
    )]
    pub letting: Box<Account<'info, PropertyLetting>>,

    /// The property, owned by the marketplace; names the location whose
    /// assignment count drops.
    #[account(
        seeds = [marketplace::PROPERTY_SEED, &asset_id.to_le_bytes()],
        bump = property.bump,
        seeds::program = marketplace::ID,
    )]
    pub property: Box<Account<'info, PropertyAsset>>,

    /// The resigning agent's registry entry. It must still exist: the
    /// assignment blocks the agent from leaving the location until this
    /// crank releases it.
    #[account(
        mut,
        seeds = [AGENT_SEED, notice.agent.as_ref()],
        bump = agent_entry.bump,
    )]
    pub agent_entry: Box<Account<'info, LettingAgent>>,

    #[account(
        mut,
        close = rent_payer,
        seeds = [RESIGNATION_SEED, &asset_id.to_le_bytes()],
        bump = notice.bump,
    )]
    pub notice: Box<Account<'info, ResignationNotice>>,
}

pub fn finalize_resignation_handler(
    ctx: Context<FinalizeResignation>,
    asset_id: u64,
) -> Result<()> {
    let notice = &ctx.accounts.notice;
    require!(
        Clock::get()?.unix_timestamp >= notice.due_ts,
        PropertyError::NoticePeriodRunning
    );
    // The seat can't have changed hands while the notice ran: claims are
    // blocked while an agent is assigned, and only this crank unassigns.
    require!(
        ctx.accounts.letting.agent == notice.agent,
        PropertyError::NotAssignedAgent
    );

    let location = &ctx.accounts.property.location;
    let entry = &mut ctx.accounts.agent_entry;
    let entry_location = entry
        .locations
        .iter_mut()
        .find(|l| l.postcode == *location)
        .ok_or(PropertyError::NotInLocation)?;
    entry_location.assigned_count = entry_location
        .assigned_count
        .checked_sub(1)
        .ok_or(PropertyError::Overflow)?;

    ctx.accounts.letting.agent = Pubkey::default();

    emit!(AgentResignationFinalized {
        asset_id,
        agent: notice.agent,
    });
    Ok(())
}

#[event]
pub struct AgentResignationInitiated {
    pub asset_id: u64,
    pub agent: Pubkey,
    pub due_ts: i64,
}

#[event]
pub struct AgentResignationFinalized {
    pub asset_id: u64,
    pub agent: Pubkey,
}
