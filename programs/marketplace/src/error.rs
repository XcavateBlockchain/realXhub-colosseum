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
}
