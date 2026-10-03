use anchor_lang::prelude::*;
use anchor_spl::associated_token::{self, AssociatedToken, Create};
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::constants::*;
use crate::error::HubError;
use crate::state::{
    Config, EvidenceStatus, Hub, HubFunding, HubMilestone, HubStatus, MilestonePolicy,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct MilestonePolicyParams {
    pub assessor: Pubkey,
    pub approval_threshold_bps: u16,
    pub assessment_policy_uri: String,
    pub assessment_policy_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct EvidenceParams {
    pub evidence_uri: String,
    pub evidence_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AssessmentParams {
    pub revision: u32,
    pub evidence_hash: [u8; 32],
    pub assessment_policy_hash: [u8; 32],
    pub probability_bps: u16,
    pub assessment_uri: String,
    pub assessment_hash: [u8; 32],
}

#[derive(Accounts)]
pub struct InitializeMilestonePolicy<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = authority @ HubError::NotUpgradeAuthority)]
    pub config: Box<Account<'info, Config>>,
    #[account(init, payer = authority, space = 8 + MilestonePolicy::INIT_SPACE,
        seeds = [MILESTONE_POLICY_SEED], bump)]
    pub policy: Box<Account<'info, MilestonePolicy>>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_policy_handler(
    ctx: Context<InitializeMilestonePolicy>,
    params: MilestonePolicyParams,
) -> Result<()> {
    require!(
        params.assessor != Pubkey::default()
            && params.approval_threshold_bps > 0
            && params.approval_threshold_bps <= PROBABILITY_SCALE
            && valid_reference(
                &params.assessment_policy_uri,
                &params.assessment_policy_hash
            ),
        HubError::InvalidMilestonePolicy
    );
    let policy = &mut ctx.accounts.policy;
    policy.assessor = params.assessor;
    policy.approval_threshold_bps = params.approval_threshold_bps;
    policy.assessment_policy_uri = params.assessment_policy_uri;
    policy.assessment_policy_hash = params.assessment_policy_hash;
    policy.bump = ctx.bumps.policy;
    emit!(MilestonePolicyInitialized {
        assessor: policy.assessor,
        approval_threshold_bps: policy.approval_threshold_bps,
        assessment_policy_hash: policy.assessment_policy_hash,
        window_seconds: MILESTONE_WINDOW_SECONDS,
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct SubmitEvidence<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump,
        has_one = operator @ HubError::NotOperator)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(seeds = [FUNDING_SEED, &hub_id.to_le_bytes()], bump = funding.bump,
        has_one = hub @ HubError::InvalidPosition)]
    pub funding: Box<Account<'info, HubFunding>>,
    #[account(seeds = [MILESTONE_POLICY_SEED], bump = policy.bump)]
    pub policy: Box<Account<'info, MilestonePolicy>>,
    #[account(init_if_needed, payer = operator, space = 8 + HubMilestone::INIT_SPACE,
        seeds = [MILESTONE_SEED, &hub_id.to_le_bytes()], bump)]
    pub milestone: Box<Account<'info, HubMilestone>>,
    pub system_program: Program<'info, System>,
}

pub fn submit_evidence_handler(
    ctx: Context<SubmitEvidence>,
    hub_id: u64,
    params: EvidenceParams,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let deadline = approval_deadline(&ctx.accounts.hub, &ctx.accounts.funding, now)?;
    require!(now < deadline, HubError::MilestoneExpired);
    require!(
        ctx.accounts.policy.assessor != ctx.accounts.operator.key(),
        HubError::SelfReview
    );
    require!(
        valid_reference(&params.evidence_uri, &params.evidence_hash),
        HubError::InvalidEvidence
    );
    let milestone = &mut ctx.accounts.milestone;
    require!(
        !milestone.second_tranche_released,
        HubError::SecondTrancheAlreadyReleased
    );
    if milestone.hub == Pubkey::default() {
        milestone.hub = ctx.accounts.hub.key();
        milestone.policy = ctx.accounts.policy.key();
        milestone.deadline = deadline;
        milestone.bump = ctx.bumps.milestone;
    }
    require!(
        milestone.hub == ctx.accounts.hub.key()
            && milestone.policy == ctx.accounts.policy.key()
            && milestone.deadline == deadline,
        HubError::InvalidPosition
    );
    milestone.revision = milestone
        .revision
        .checked_add(1)
        .ok_or(HubError::Overflow)?;
    milestone.evidence_uri = params.evidence_uri;
    milestone.evidence_hash = params.evidence_hash;
    milestone.submitted_at = now;
    milestone.status = EvidenceStatus::Submitted;
    milestone.probability_bps = 0;
    milestone.assessment_uri.clear();
    milestone.assessment_hash = [0; 32];
    milestone.assessed_at = 0;
    emit!(EvidenceSubmitted {
        hub_id,
        revision: milestone.revision,
        evidence_hash: milestone.evidence_hash,
        deadline,
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(hub_id: u64)]
pub struct AssessEvidence<'info> {
    #[account(mut)]
    pub assessor: Signer<'info>,
    #[account(seeds = [MILESTONE_POLICY_SEED], bump = policy.bump,
        has_one = assessor @ HubError::NotAssessor)]
    pub policy: Box<Account<'info, MilestonePolicy>>,
    #[account(seeds = [HUB_SEED, &hub_id.to_le_bytes()], bump = hub.bump)]
    pub hub: Box<Account<'info, Hub>>,
    #[account(seeds = [FUNDING_SEED, &hub_id.to_le_bytes()], bump = funding.bump,
        has_one = hub @ HubError::InvalidPosition)]
    pub funding: Box<Account<'info, HubFunding>>,
    #[account(mut, seeds = [MILESTONE_SEED, &hub_id.to_le_bytes()], bump = milestone.bump,
        has_one = hub @ HubError::InvalidPosition, has_one = policy @ HubError::InvalidPosition)]
    pub milestone: Box<Account<'info, HubMilestone>>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(address = config.payment_mint @ HubError::InvalidMint)]
    pub payment_mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [PAYMENT_VAULT_SEED, &hub_id.to_le_bytes()], bump,
        token::mint = payment_mint, token::authority = hub)]
    pub payment_vault: Box<Account<'info, TokenAccount>>,
    /// CHECK: Only the hub's recorded operator can receive the second tranche.
    #[account(address = hub.operator @ HubError::NotOperator)]
    pub operator: UncheckedAccount<'info>,
    /// CHECK: Canonical ATA checked in the handler; the ATA program validates it
    /// before the SPL Token transfer. An unapproved score does not create it.
    #[account(mut)]
    pub operator_payment: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn assess_evidence_handler(
    ctx: Context<AssessEvidence>,
    hub_id: u64,
    params: AssessmentParams,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let hub = &ctx.accounts.hub;
    let deadline = approval_deadline(hub, &ctx.accounts.funding, now)?;
    let milestone = &mut ctx.accounts.milestone;
    require!(
        !milestone.second_tranche_released,
        HubError::SecondTrancheAlreadyReleased
    );
    require!(now < deadline, HubError::MilestoneExpired);
    require!(milestone.deadline == deadline, HubError::InvalidPosition);
    require!(
        milestone.status == EvidenceStatus::Submitted,
        HubError::InvalidStatus
    );
    require!(
        hub.operator != ctx.accounts.assessor.key(),
        HubError::SelfReview
    );
    require!(
        milestone.revision == params.revision
            && milestone.evidence_hash == params.evidence_hash
            && ctx.accounts.policy.assessment_policy_hash == params.assessment_policy_hash,
        HubError::EvidenceMismatch
    );
    require!(
        params.probability_bps <= PROBABILITY_SCALE,
        HubError::InvalidProbability
    );
    require!(
        valid_reference(&params.assessment_uri, &params.assessment_hash),
        HubError::InvalidEvidence
    );
    require_keys_eq!(
        ctx.accounts.operator_payment.key(),
        associated_token::get_associated_token_address(
            &hub.operator,
            &ctx.accounts.payment_mint.key()
        ),
        HubError::InvalidPosition
    );
    // The assessor attests a probability; the program computes the decision.
    // There is no approve/reject override or community voting instruction.
    let approved = params.probability_bps >= ctx.accounts.policy.approval_threshold_bps;
    if approved {
        let amount = hub
            .total_paid
            .checked_sub(ctx.accounts.funding.first_tranche_amount)
            .ok_or(HubError::Overflow)?;
        associated_token::create_idempotent(CpiContext::new(
            ctx.accounts.associated_token_program.key(),
            Create {
                payer: ctx.accounts.assessor.to_account_info(),
                associated_token: ctx.accounts.operator_payment.to_account_info(),
                authority: ctx.accounts.operator.to_account_info(),
                mint: ctx.accounts.payment_mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            },
        ))?;
        let id = hub_id.to_le_bytes();
        let bump = [hub.bump];
        let seeds: &[&[u8]] = &[HUB_SEED, &id, &bump];
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.payment_vault.to_account_info(),
                    mint: ctx.accounts.payment_mint.to_account_info(),
                    to: ctx.accounts.operator_payment.to_account_info(),
                    authority: hub.to_account_info(),
                },
                &[seeds],
            ),
            amount,
            ctx.accounts.payment_mint.decimals,
        )?;
        milestone.second_tranche_amount = amount;
        milestone.second_tranche_released = true;
        milestone.second_tranche_released_at = now;
        emit!(SecondTrancheReleased {
            hub_id,
            operator: hub.operator,
            amount,
            released_at: now
        });
    }
    milestone.status = if approved {
        EvidenceStatus::Approved
    } else {
        EvidenceStatus::Rejected
    };
    milestone.probability_bps = params.probability_bps;
    milestone.assessment_uri = params.assessment_uri;
    milestone.assessment_hash = params.assessment_hash;
    milestone.assessed_at = now;
    emit!(EvidenceAssessed {
        hub_id,
        revision: milestone.revision,
        evidence_hash: milestone.evidence_hash,
        assessment_policy_hash: ctx.accounts.policy.assessment_policy_hash,
        assessment_hash: milestone.assessment_hash,
        assessor: ctx.accounts.assessor.key(),
        probability_bps: milestone.probability_bps,
        approved,
    });
    Ok(())
}

fn valid_reference(uri: &str, hash: &[u8; 32]) -> bool {
    !uri.trim().is_empty() && uri.len() <= 200 && *hash != [0; 32]
}

pub(super) fn approval_deadline(hub: &Hub, funding: &HubFunding, now: i64) -> Result<i64> {
    require!(
        funding.first_tranche_released,
        HubError::FirstTrancheRequired
    );
    require!(
        hub.status == HubStatus::Funded
            && hub.tokens_sold == hub.token_supply
            && hub.tokens_claimed == hub.token_supply
            && hub.total_refunded == 0
            && now >= funding.first_tranche_released_at,
        HubError::InvalidStatus
    );
    let target = hub
        .token_price
        .checked_mul(hub.token_supply)
        .ok_or(HubError::Overflow)?;
    require!(
        target > 0 && hub.total_paid == target && funding.first_tranche_amount == target / 2,
        HubError::InvalidPosition
    );
    funding
        .first_tranche_released_at
        .checked_add(MILESTONE_WINDOW_SECONDS)
        .ok_or_else(|| error!(HubError::Overflow))
}

#[event]
pub struct MilestonePolicyInitialized {
    pub assessor: Pubkey,
    pub approval_threshold_bps: u16,
    pub assessment_policy_hash: [u8; 32],
    pub window_seconds: i64,
}
#[event]
pub struct EvidenceSubmitted {
    pub hub_id: u64,
    pub revision: u32,
    pub evidence_hash: [u8; 32],
    pub deadline: i64,
}
#[event]
pub struct EvidenceAssessed {
    pub hub_id: u64,
    pub revision: u32,
    pub evidence_hash: [u8; 32],
    pub assessment_policy_hash: [u8; 32],
    pub assessment_hash: [u8; 32],
    pub assessor: Pubkey,
    pub probability_bps: u16,
    pub approved: bool,
}
#[event]
pub struct SecondTrancheReleased {
    pub hub_id: u64,
    pub operator: Pubkey,
    pub amount: u64,
    pub released_at: i64,
}
