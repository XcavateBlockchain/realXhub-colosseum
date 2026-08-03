use anchor_lang::prelude::*;

#[error_code]
pub enum MarketplaceError {
    /// The signer is not the configured authority.
    #[msg("Signer is not the authority")]
    NotAuthority,
    /// The supplied config parameters are invalid.
    #[msg("Invalid config parameters")]
    InvalidConfig,
    /// The signer is not the program's upgrade authority.
    #[msg("Signer is not the program upgrade authority")]
    NotUpgradeAuthority,
    /// The signer does not match the pending authority proposal.
    #[msg("Signer is not the pending authority")]
    NotPendingAuthority,
    /// The mint carries a token extension the vault accounting cannot support.
    #[msg("Unsupported token extension on mint")]
    UnsupportedMintExtension,
    /// The supplied mint is not the configured XCAV mint.
    #[msg("Invalid XCAV mint")]
    InvalidMint,
    /// The lawyer still has active cases and can't unregister.
    #[msg("Lawyer still has active cases")]
    LawyerStillActive,
    /// The role exists but its compliance flag is not set.
    #[msg("Role is not compliant")]
    NotCompliant,
    /// The share amount is zero or outside the configured bounds.
    #[msg("Invalid share amount")]
    InvalidShareAmount,
    /// The share price is zero.
    #[msg("Invalid share price")]
    InvalidSharePrice,
    /// The listing's expiry has passed.
    #[msg("Listing has expired")]
    ListingExpired,
    /// The signer is not the listing's developer.
    #[msg("Signer is not the listing developer")]
    NotListingDeveloper,
    /// Every share has been sold, or the legal phase has already started.
    #[msg("Property is already sold")]
    PropertyAlreadySold,
    /// The listing is not open for this action.
    #[msg("Listing is not active")]
    ListingNotActive,
    /// Arithmetic overflow.
    #[msg("Arithmetic overflow")]
    Overflow,
    /// The mint has an authority that could lock vaulted funds.
    #[msg("Unsupported mint authority")]
    UnsupportedMintAuthority,
    /// The computed deposit is above the caller's stated maximum.
    #[msg("Deposit exceeds the caller's maximum")]
    DepositTooHigh,
    /// The payment mint is not on the accepted list.
    #[msg("Payment mint is not accepted")]
    MintNotAccepted,
    /// The position was cancelled by an unreserve; re-buying is barred.
    #[msg("Position was cancelled and cannot buy again")]
    PositionCancelled,
    /// The purchase would push the investor over the ownership cap.
    #[msg("Purchase exceeds the ownership cap")]
    MaxOwnershipExceeded,
    /// The total cost is above the caller's stated maximum.
    #[msg("Cost exceeds the caller's maximum")]
    CostTooHigh,
    /// The position was paid in a different mint.
    #[msg("Position uses a different payment mint")]
    PaymentMintMismatch,
    /// The rent payer is not the configured rent collector.
    #[msg("Payer is not the rent collector")]
    NotRentCollector,
    /// The position holds no shares to return.
    #[msg("Nothing to unreserve")]
    NothingToUnreserve,
    /// Only cancelled positions can be reclaimed by the crank.
    #[msg("Position is not cancelled")]
    PositionNotCancelled,
    /// The listing is still selling, so the position must stay open.
    #[msg("Listing is still active")]
    ListingStillActive,
    /// The sale is not fully reserved yet.
    #[msg("Not every share is reserved")]
    NotFullyReserved,
    /// The SPV was already confirmed for this property.
    #[msg("SPV already created")]
    SpvAlreadyCreated,
    /// The listing has not reached its expiry.
    #[msg("Listing is not expired")]
    ListingNotExpired,
    /// Shares are still held by investors.
    #[msg("Shares are still outstanding")]
    SharesOutstanding,
    /// The listing deposit was already withdrawn.
    #[msg("Deposit already withdrawn")]
    DepositAlreadyWithdrawn,
    /// The ownership cap would not admit a single purchase.
    #[msg("Ownership cap admits no purchase")]
    OwnershipCapTooTight,
    /// The share ledger does not match the position.
    #[msg("Share ledger does not match the position")]
    LedgerMismatch,
    /// Shares are locked by a vote.
    #[msg("Shares are locked")]
    SharesLocked,
    /// The legal process still has time to run.
    #[msg("Legal process has not expired")]
    LegalProcessNotExpired,
    /// The developer has not withdrawn the listing deposit yet.
    #[msg("Deposit still held")]
    DepositStillHeld,
    /// Investor positions are still open, cancelled ones included.
    #[msg("Positions still open")]
    PositionsOutstanding,
    /// The account is not the vault's associated account for this mint.
    #[msg("Wrong vault account")]
    WrongVaultAccount,
    /// The legal process deadline has passed.
    #[msg("Legal process has expired")]
    LegalProcessExpired,
    /// This side of the case already has a lawyer.
    #[msg("Case already has a lawyer for this side")]
    LawyerJobTaken,
    /// The same lawyer cannot act for both sides of a sale.
    #[msg("Lawyer cannot represent both sides")]
    ConflictOfInterest,
    /// The combined lawyer costs would exceed the collected fees.
    #[msg("Lawyer costs exceed the collected fees")]
    CostsExceedFees,
    /// The lawyer serves a different region than the property.
    #[msg("Lawyer is registered for a different region")]
    WrongRegion,
    /// No election is open to act on.
    #[msg("No lawyer proposed")]
    NoLawyerProposed,
    /// The SPV must exist before its lawyer can be elected.
    #[msg("SPV has not been created")]
    SpvNotCreated,
    /// The vote amount is zero.
    #[msg("Invalid vote amount")]
    InvalidVoteAmount,
    /// The voter holds fewer unlocked shares than the vote needs.
    #[msg("Not enough unlocked shares")]
    NotEnoughShares,
    /// The election is still open.
    #[msg("Voting is still ongoing")]
    VotingStillOngoing,
    /// The election has closed.
    #[msg("Voting has closed")]
    VotingClosed,
    /// The signer is not a lawyer on this case.
    #[msg("Signer is not a lawyer on this case")]
    NotCaseLawyer,
    /// The documents were already confirmed, so the lawyer is committed.
    #[msg("Documents already confirmed")]
    AlreadyConfirmed,
    /// The round does not match the election's current state.
    #[msg("Wrong election round")]
    WrongElectionRound,
    /// The election already has the most candidates it can carry.
    #[msg("Too many candidates")]
    TooManyCandidates,
    /// A candidacy account does not belong to this election round.
    #[msg("Candidacy does not match the election")]
    CandidacyMismatch,
    /// The account does not belong to the lawyer it should.
    #[msg("Wrong lawyer account")]
    WrongLawyer,
    /// A lawyer is still engaged on the case and must resign first.
    #[msg("Lawyer still engaged on the case")]
    LawyerStillEngaged,
    /// The claim window has closed.
    #[msg("Claim window has closed")]
    ClaimWindowClosed,
    /// Direct purchases open once the claim window has run out.
    #[msg("Direct purchase is not open yet")]
    DirectBuyNotOpen,
    /// The position has no reserved shares.
    #[msg("Nothing reserved")]
    NothingReserved,
    /// The payment account holds less than the reservation needs.
    #[msg("Balance too low to reserve")]
    BalanceTooLow,
    /// The position still has unclaimed reserved shares.
    #[msg("Reservation outstanding")]
    ReservationOutstanding,
    /// Backing out ends once every share is reserved.
    #[msg("The sale is locked in")]
    SaleLocked,
}
