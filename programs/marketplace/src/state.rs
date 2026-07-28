use anchor_lang::prelude::*;

pub use regions::state::POSTCODE_MAX_LEN;

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
    /// Mints a property can be priced and paid in. Every entry must be a
    /// same-value GBP stablecoin: prices convert between them by decimal
    /// count alone, so adding a mint of different value would misprice every
    /// open listing.
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

/// Where a primary listing is in its lifecycle.
#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListingStatus {
    /// Created, but the Core asset and share mint don't exist yet.
    PendingAssets,
    /// Open for share purchases.
    Listed,
    /// Every share sold; the legal process can start.
    SoldOut,
    /// Lawyers are confirming the sale documents.
    Legal,
    /// Settled; the property is live.
    Finalized,
    /// Expired before selling out; refunds open.
    Expired,
    /// Cancelled by the legal process; refunds open.
    Cancelled,
    /// Being torn down; waiting for the last holder to withdraw.
    Refunding,
}

/// A fractionalized property. Created when it is listed and kept for the
/// asset's whole life; the Core asset and share mint are attached by
/// `init_property_assets`.
#[account]
#[derive(InitSpace)]
pub struct PropertyAsset {
    pub asset_id: u64,
    /// The Metaplex Core asset held in the property vault. Default until the
    /// assets are initialized.
    pub core_asset: Pubkey,
    /// The Token-2022 share mint. Default until the assets are initialized.
    pub share_mint: Pubkey,
    pub region_id: u16,
    /// The registered location (postcode) the property sits in.
    #[max_len(POSTCODE_MAX_LEN)]
    pub location: Vec<u8>,
    pub share_amount: u32,
    pub spv_created: bool,
    pub finalized: bool,
    /// Wallets currently holding shares; teardown completes when it reaches
    /// zero again.
    pub holder_count: u32,
    pub bump: u8,
}

/// Where a lawyer's document review stands.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, PartialEq, Eq, Debug, Default,
)]
pub enum DocumentStatus {
    #[default]
    Pending,
    Approved,
    Rejected,
}

/// One side's engaged lawyer on a sold-out sale.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, PartialEq, Eq, Debug, Default,
)]
pub struct LawyerAssignment {
    /// The lawyer on this side; default until one is engaged.
    pub lawyer: Pubkey,
    /// What they charge, quoted at `PRICE_DECIMALS`, paid out of the
    /// collected investor fees at settlement.
    pub costs: u64,
    pub doc_status: DocumentStatus,
}

/// Most lawyers that can stand in one SPV election round. Bounds the account
/// list `finalize_spv_election` walks to find the plurality winner.
pub const MAX_SPV_CANDIDATES: u32 = 5;

/// The running SPV lawyer election. Any eligible lawyer may stand; the first
/// candidacy opens the voting window and shareholders vote among the
/// candidates. A round that elects nobody simply reopens: rounds repeat
/// until a lawyer is engaged or the legal process expires into its timeout
/// exit. One round at a time per listing.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, PartialEq, Eq, Debug, Default,
)]
pub struct SpvElection {
    /// When voting closes; zero while no round is running.
    pub expiry: i64,
    /// Candidates standing in the current round.
    pub candidate_count: u32,
    /// Round number, monotonic per listing. Candidacies and vote records are
    /// keyed by it, so nothing stale can count toward a later round.
    pub round: u64,
}

/// A primary property listing. Prices and windows are snapshotted here at
/// listing time, so config or region changes never reprice a sale underway.
#[account]
#[derive(InitSpace)]
pub struct Listing {
    pub listing_id: u64,
    pub developer: Pubkey,
    pub asset_id: u64,
    /// Price per share, in payment-mint base units.
    pub share_price: u64,
    /// Shares put up for sale.
    pub listed_share_amount: u32,
    /// Shares sold so far.
    pub sold_share_amount: u32,
    /// Whether the developer covers the region's sale tax themselves.
    pub tax_paid_by_developer: bool,
    /// The region's sale tax at listing time, in basis points.
    pub tax_bps: u16,
    /// Protocol fee at listing time, in basis points.
    pub marketplace_fee_bps: u16,
    /// Investor fee at listing time, in basis points.
    pub investor_fee_bps: u16,
    /// Ownership cap at listing time, in basis points. Holdings must stay
    /// strictly below it, so even 10_000 requires at least two holders.
    pub max_ownership_bps: u16,
    pub listing_expiry: i64,
    /// Seconds the legal process may run once the listing sells out, taken
    /// from config at listing time.
    pub legal_process_time: i64,
    /// Seconds the SPV-lawyer election stays open, taken from config at
    /// listing time.
    pub lawyer_voting_time: i64,
    /// Quorum for the SPV-lawyer election, taken from config at listing time.
    pub min_voting_quorum_bps: u16,
    /// Open `InvestorPosition` accounts, cancelled ones included. Teardown
    /// waits for zero, so a position can never outlive the listing it needs
    /// to close against.
    pub position_count: u32,
    /// Set when the last share sells: the moment the legal process runs out
    /// and the timeout exit opens. Zero until then.
    pub legal_deadline: i64,
    /// The XCAV locked by the developer at listing, held in the vault. Zero
    /// doubles as "already withdrawn", which is unambiguous because config
    /// validation never accepts a zero deposit.
    pub deposit: u64,
    /// The developer's lawyer on the sale, allocated by the developer.
    pub developer_lawyer: LawyerAssignment,
    /// The SPV's lawyer on the sale, chosen by investor vote.
    pub spv_lawyer: LawyerAssignment,
    pub spv_election: SpvElection,
    pub status: ListingStatus,
    pub bump: u8,
}

impl Listing {
    /// The investor fees a full sale collects, quoted at `PRICE_DECIMALS`.
    /// Lawyer costs are capped by this pot; the per-buy transfers floor when
    /// rescaling to each mint, so settlement pays out with the same floor.
    pub fn total_fee_quote(&self) -> Result<u64> {
        let gross = self.share_price as u128 * self.listed_share_amount as u128;
        u64::try_from(gross * self.investor_fee_bps as u128 / 10_000)
            .map_err(|_| crate::error::MarketplaceError::Overflow.into())
    }
}

/// One lawyer standing in one SPV election round; carries their own tally.
/// The lawyer pays the rent and takes it back with `close_candidacy` once
/// the round is over.
#[account]
#[derive(InitSpace)]
pub struct LawyerCandidacy {
    pub listing_id: u64,
    /// The election round the candidacy belongs to.
    pub round: u64,
    pub lawyer: Pubkey,
    /// What the candidate would charge, quoted at `PRICE_DECIMALS`.
    pub costs: u64,
    /// Share-weighted votes cast for this candidate.
    pub vote_power: u32,
    pub bump: u8,
}

/// One investor's vote in one SPV lawyer election round. Locks the voting
/// shares until the record is closed again by `unlock_voting_shares`.
#[account]
#[derive(InitSpace)]
pub struct LawyerVote {
    pub listing_id: u64,
    /// The election round the vote belongs to.
    pub round: u64,
    pub voter: Pubkey,
    /// The candidate voted for.
    pub choice: Pubkey,
    /// The shares this vote locked and counts for.
    pub power: u32,
    pub bump: u8,
}

/// Prices are quoted at this scale (tGBP's 9 decimals); transfers rescale to
/// each payment mint's own decimals, flooring in the investor's favour. The
/// rescale is by decimal count alone, which is only sound because every
/// accepted payment mint must be a same-value GBP stablecoin.
pub const PRICE_DECIMALS: u8 = 9;

/// Accepted payment mints must sit in this decimals range: the lower bound
/// keeps a minimum-priced share from flooring to zero, the upper bound keeps
/// the rescale arithmetic comfortably inside u128.
pub const MIN_PAYMENT_DECIMALS: u8 = 6;
pub const MAX_PAYMENT_DECIMALS: u8 = 12;

/// The canonical share ledger for one holder of one property. The Token-2022
/// accounts mirror this; they never lead it.
#[account]
#[derive(InitSpace)]
pub struct ShareHolding {
    pub asset_id: u64,
    pub owner: Pubkey,
    pub amount: u32,
    /// Shares locked by votes; blocked from transfer until unlocked.
    pub locked_amount: u32,
    pub bump: u8,
}

/// One investor's stake in a primary listing: what they paid, per component,
/// so refunds and settlement can be exact. The shares themselves are already
/// delivered; this is the accounting.
#[account]
#[derive(InitSpace)]
pub struct InvestorPosition {
    pub listing_id: u64,
    pub investor: Pubkey,
    /// The mint the investor paid in. One mint per position: later buys must
    /// use the same one, so every refund is a single transfer.
    pub payment_mint: Pubkey,
    pub share_amount: u32,
    /// Paid toward the property price, in payment-mint base units.
    pub paid_funds: u64,
    /// Paid as sale tax on top, in payment-mint base units.
    pub paid_tax: u64,
    /// Paid as the investor fee on top, in payment-mint base units.
    pub paid_fee: u64,
    /// Set when the investor unreserved. The position stays open as the
    /// one-way re-buy bar: `buy` rejects a cancelled position forever.
    pub cancelled: bool,
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
