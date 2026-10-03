//! Hub proposals, escrowed purchases, and optional unpaid reservations. Paid
//! sales retain their original settlement path; reservations use separate records.

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;
pub use constants::*;
use instructions::*;
pub use instructions::{
    AssessmentParams, ConfigParams, EvidenceParams, MilestonePolicyParams, ProposalParams,
};

declare_id!("HbHu1p5KJJsCehawX5NBdqZXuGzHyqyPAsZcYUUz21b");

#[program]
pub mod realxhub {
    use super::*;

    pub fn initialize_config(ctx: Context<InitializeConfig>, params: ConfigParams) -> Result<()> {
        initialize::handler(ctx, params)
    }
    pub fn create_proposal(
        ctx: Context<CreateProposal>,
        region_id: u16,
        params: ProposalParams,
    ) -> Result<()> {
        proposal::create_proposal_handler(ctx, region_id, params)
    }
    pub fn update_proposal(
        ctx: Context<OperatorProposal>,
        hub_id: u64,
        params: ProposalParams,
    ) -> Result<()> {
        proposal::update_proposal_handler(ctx, hub_id, params)
    }
    pub fn submit_proposal(ctx: Context<OperatorProposal>, hub_id: u64) -> Result<()> {
        proposal::submit_proposal_handler(ctx, hub_id)
    }
    pub fn review_proposal(
        ctx: Context<ReviewProposal>,
        hub_id: u64,
        revision: u32,
        metadata_hash: [u8; 32],
        approve: bool,
    ) -> Result<()> {
        proposal::review_proposal_handler(ctx, hub_id, revision, metadata_hash, approve)
    }
    pub fn activate_hub(ctx: Context<ActivateHub>, hub_id: u64, max_bond: u64) -> Result<()> {
        activation::activate_hub_handler(ctx, hub_id, max_bond)
    }
    pub fn buy_tokens(
        ctx: Context<BuyTokens>,
        hub_id: u64,
        amount: u64,
        max_total_cost: u64,
    ) -> Result<()> {
        purchase::buy_tokens_handler(ctx, hub_id, amount, max_total_cost)
    }
    pub fn finalize_sale(ctx: Context<FinalizeSale>, hub_id: u64) -> Result<()> {
        purchase::finalize_sale_handler(ctx, hub_id)
    }
    pub fn claim_tokens(ctx: Context<ClaimTokens>, hub_id: u64) -> Result<()> {
        purchase::claim_tokens_handler(ctx, hub_id)
    }
    pub fn refund_purchase(ctx: Context<RefundPurchase>, hub_id: u64) -> Result<()> {
        purchase::refund_purchase_handler(ctx, hub_id)
    }
    pub fn refund_bond(ctx: Context<RefundBond>, hub_id: u64) -> Result<()> {
        activation::refund_bond_handler(ctx, hub_id)
    }

    pub fn open_reservations(ctx: Context<OpenReservations>, hub_id: u64) -> Result<()> {
        reservation::open_reservations_handler(ctx, hub_id)
    }

    pub fn reserve_tokens(
        ctx: Context<ReserveTokens>,
        hub_id: u64,
        amount: u64,
        max_total_cost: u64,
    ) -> Result<()> {
        reservation::reserve_tokens_handler(ctx, hub_id, amount, max_total_cost)
    }

    pub fn cancel_reservation(ctx: Context<CancelReservation>, hub_id: u64) -> Result<()> {
        reservation::cancel_reservation_handler(ctx, hub_id)
    }

    pub fn claim_reserved_tokens(
        ctx: Context<ClaimReservedTokens>,
        hub_id: u64,
        max_total_cost: u64,
    ) -> Result<()> {
        reservation_claim::claim_reserved_tokens_handler(ctx, hub_id, max_total_cost)
    }

    pub fn release_reservation(
        ctx: Context<ReleaseReservation>,
        hub_id: u64,
        buyer: Pubkey,
    ) -> Result<()> {
        reservation::release_reservation_handler(ctx, hub_id, buyer)
    }

    pub fn release_first_tranche(ctx: Context<ReleaseFirstTranche>, hub_id: u64) -> Result<()> {
        funding::release_first_tranche_handler(ctx, hub_id)
    }

    pub fn reopen_reservations(ctx: Context<ReopenReservations>, hub_id: u64) -> Result<()> {
        reservation::reopen_reservations_handler(ctx, hub_id)
    }

    pub fn finalize_reservations(ctx: Context<FinalizeReservations>, hub_id: u64) -> Result<()> {
        reservation::finalize_reservations_handler(ctx, hub_id)
    }
    pub fn initialize_milestone_policy(
        ctx: Context<InitializeMilestonePolicy>,
        params: MilestonePolicyParams,
    ) -> Result<()> {
        milestone::initialize_policy_handler(ctx, params)
    }

    pub fn submit_evidence(
        ctx: Context<SubmitEvidence>,
        hub_id: u64,
        params: EvidenceParams,
    ) -> Result<()> {
        milestone::submit_evidence_handler(ctx, hub_id, params)
    }

    pub fn assess_evidence(
        ctx: Context<AssessEvidence>,
        hub_id: u64,
        params: AssessmentParams,
    ) -> Result<()> {
        milestone::assess_evidence_handler(ctx, hub_id, params)
    }

    pub fn declare_default(ctx: Context<DeclareDefault>, hub_id: u64) -> Result<()> {
        default::declare_default_handler(ctx, hub_id)
    }

    pub fn redeem_default(
        ctx: Context<RedeemDefault>,
        hub_id: u64,
        amount: u64,
        min_payment_out: u64,
        min_xcav_out: u64,
    ) -> Result<()> {
        default::redeem_default_handler(ctx, hub_id, amount, min_payment_out, min_xcav_out)
    }
}
