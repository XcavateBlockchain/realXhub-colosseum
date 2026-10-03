use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    pub verifier: Pubkey,
    pub xcav_mint: Pubkey,
    pub payment_mint: Pubkey,
    pub bond_amount: u64,
    pub next_hub_id: u64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HubStatus {
    Draft,
    Submitted,
    Approved,
    Rejected,
    Listed,
    Funded,
    Failed,
    // Append phases so existing Hub accounts keep their original status tags.
    Reserving,
    Claiming,
    Defaulted,
}

#[account]
#[derive(InitSpace)]
pub struct Hub {
    pub hub_id: u64,
    pub operator: Pubkey,
    pub region_id: u16,
    #[max_len(64)]
    pub name: String,
    #[max_len(200)]
    pub metadata_uri: String,
    pub metadata_hash: [u8; 32],
    pub revision: u32,
    pub status: HubStatus,
    pub submitted_at: i64,
    pub reviewed_at: i64,
    pub reviewed_by: Pubkey,
    /// Price in the configured payment mint's smallest units per whole token.
    pub token_price: u64,
    pub token_supply: u64,
    pub sale_duration: i64,
    pub sale_deadline: i64,
    /// Snapshotted when the draft is created, not recomputed at activation.
    pub bond_amount: u64,
    pub bond_refunded: bool,
    pub token_mint: Pubkey,
    pub tokens_sold: u64,
    pub total_paid: u64,
    pub total_refunded: u64,
    pub tokens_claimed: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct BuyerPosition {
    pub hub: Pubkey,
    pub buyer: Pubkey,
    pub amount: u64,
    pub paid: u64,
    pub settled: bool,
    pub bump: u8,
}

/// Unpaid reservations are separate from the existing paid-sale accounts.
/// Opting into this flow never reinterprets an old buyer's recorded payment.
#[account]
#[derive(InitSpace)]
pub struct ReservationSale {
    pub hub: Pubkey,
    /// Pending unpaid quantities; delivered quantities live in Hub.tokens_sold.
    pub reserved_tokens: u64,
    pub claim_started_at: i64,
    pub claim_deadline: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct HubReservation {
    pub hub: Pubkey,
    pub buyer: Pubkey,
    /// Claims must collect from the account against which the promise was made.
    pub payment_account: Pubkey,
    pub amount: u64,
    /// Payment-mint base units promised, not money held by the program.
    pub quoted_payment: u64,
    pub bump: u8,
}

/// Sum of unpaid hub promises against one payment account. This does not lock
/// the wallet or include reservations owned by the separate marketplace program.
#[account]
#[derive(InitSpace)]
pub struct PaymentReservation {
    pub payment_account: Pubkey,
    pub amount: u64,
    pub bump: u8,
}

/// A paid reservation receipt. Separate from legacy purchase positions so
/// token delivery cannot be confused with an unclaimed paid allocation.
#[account]
#[derive(InitSpace)]
pub struct ReservationClaim {
    pub hub: Pubkey,
    pub buyer: Pubkey,
    pub amount: u64,
    pub paid: u64,
    pub last_claimed_at: i64,
    pub bump: u8,
}

/// Funding settlement is separate so existing hub and reservation layouts stay
/// readable. The first tranche is released exactly once after every token is claimed.
#[account]
#[derive(InitSpace)]
pub struct HubFunding {
    pub hub: Pubkey,
    pub first_tranche_amount: u64,
    pub first_tranche_released: bool,
    pub first_tranche_released_at: i64,
    pub bump: u8,
}

/// Immutable assessment identity and methodology; separate from proposal review.
#[account]
#[derive(InitSpace)]
pub struct MilestonePolicy {
    pub assessor: Pubkey,
    pub approval_threshold_bps: u16,
    #[max_len(200)]
    pub assessment_policy_uri: String,
    pub assessment_policy_hash: [u8; 32],
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvidenceStatus {
    Submitted,
    Rejected,
    Approved,
}

/// Evidence revisions, signed assessment and second-tranche receipt are separate
/// so no existing hub, first-tranche or buyer account needs reallocation.
#[account]
#[derive(InitSpace)]
pub struct HubMilestone {
    pub hub: Pubkey,
    pub policy: Pubkey,
    pub deadline: i64,
    pub revision: u32,
    #[max_len(200)]
    pub evidence_uri: String,
    pub evidence_hash: [u8; 32],
    pub submitted_at: i64,
    pub status: EvidenceStatus,
    pub probability_bps: u16,
    #[max_len(200)]
    pub assessment_uri: String,
    pub assessment_hash: [u8; 32],
    pub assessed_at: i64,
    pub second_tranche_amount: u64,
    pub second_tranche_released: bool,
    pub second_tranche_released_at: i64,
    pub bump: u8,
}

/// Default pools are fixed from recorded liabilities, not current vault balances.
#[account]
#[derive(InitSpace)]
pub struct HubDefault {
    pub hub: Pubkey,
    pub token_supply: u64,
    pub payment_pool: u64,
    pub xcav_pool: u64,
    pub tokens_redeemed: u64,
    pub payment_redeemed: u64,
    pub xcav_redeemed: u64,
    pub approval_deadline: i64,
    pub defaulted_at: i64,
    pub bump: u8,
}

/// Original buyers redeem only up to their cumulative paid claim quantity.
#[account]
#[derive(InitSpace)]
pub struct DefaultRedemption {
    pub hub: Pubkey,
    pub buyer: Pubkey,
    pub amount: u64,
    pub payment_received: u64,
    pub xcav_received: u64,
    pub bump: u8,
}
