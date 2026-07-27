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
    /// The mint carries a token extension the escrow accounting cannot support.
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
}
