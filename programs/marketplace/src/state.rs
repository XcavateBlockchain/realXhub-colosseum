use anchor_lang::prelude::*;

/// Most payment mints the protocol accepts at once.
pub const MAX_PAYMENT_MINTS: usize = 4;

/// Hard ceiling on shares per property. No instruction may iterate holders
/// (teardown is lazy, per holder), but the supply still has to stay bounded so
/// per-share arithmetic can't overflow u64 prices.
pub const MAX_SHARE_SUPPLY: u32 = 100;

/// Singleton config holding protocol parameters and the authority.
#[account]
#[derive(InitSpace)]
pub struct Config {
    /// Authority allowed to update parameters.
    pub authority: Pubkey,
    /// Proposed replacement authority; takes over via `accept_authority`.
    /// Two-step so a typo'd address can't brick parameter management.
    pub pending_authority: Option<Pubkey>,
    /// The XCAV mint listing and lawyer deposits are paid in.
    pub xcav_mint: Pubkey,
    /// Owner of the shared protocol treasury (a multisig on mainnet). Fees are
    /// paid to this key's token accounts; the program never holds them.
    pub treasury: Pubkey,
    /// The sponsor wallet that fronts account rent for investors. Every
    /// position close sends its lamports here, not to the investor.
    pub rent_collector: Pubkey,
    /// Mints a property can be priced and paid in.
    #[max_len(MAX_PAYMENT_MINTS)]
    pub accepted_payment_mints: Vec<Pubkey>,
    /// XCAV a developer locks to list a property.
    pub listing_deposit: u64,
    /// XCAV a lawyer locks to join the registry.
    pub lawyer_deposit: u64,
    /// Fewest shares a property may be split into.
    pub min_property_shares: u32,
    /// Most shares a property may be split into.
    pub max_property_shares: u32,
    /// Protocol fee taken from the developer's proceeds, in basis points.
    pub marketplace_fee_bps: u16,
    /// Fee an investor pays on top of the share price, in basis points.
    pub investor_fee_bps: u16,
    /// Largest slice of a property one investor may hold, in basis points.
    pub max_ownership_bps: u16,
    /// Seconds the legal process may run before it expires.
    pub legal_process_time: i64,
    /// Seconds the SPV lawyer election stays open once the first lawyer claims.
    pub lawyer_voting_time: i64,
    /// Share of a property's supply that must vote for the SPV lawyer election
    /// to be valid, in basis points.
    pub min_voting_quorum_bps: u16,
    /// Monotonic id for the next listing.
    pub next_listing_id: u64,
    pub bump: u8,
}

/// A lawyer registered to take cases in a region. One registration per wallet;
/// only registered lawyers can be assigned to a sale.
#[account]
#[derive(InitSpace)]
pub struct Lawyer {
    pub lawyer: Pubkey,
    /// The region the lawyer serves; cases must match it.
    pub region_id: u16,
    /// The XCAV locked at registration, held in the vault. Returned on
    /// unregister, even if the configured deposit has changed since.
    pub deposit: u64,
    /// Cases currently assigned; must be zero to unregister.
    pub active_cases: u32,
    pub bump: u8,
}
