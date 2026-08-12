use anchor_lang::prelude::*;

#[error_code]
pub enum PropertyError {
    #[msg("Invalid config parameters")]
    InvalidConfig,
    #[msg("Signer is not the config authority")]
    NotAuthority,
    #[msg("Signer is not the pending authority")]
    NotPendingAuthority,
    #[msg("Signer is not the program upgrade authority")]
    NotUpgradeAuthority,
    #[msg("Arithmetic overflow")]
    Overflow,
    /// The mint has an authority or extension the vault accounting cannot
    /// support.
    #[msg("Mint carries an unsupported authority")]
    UnsupportedMintAuthority,
    #[msg("Mint carries an unsupported extension")]
    UnsupportedMintExtension,
    #[msg("Account is not a supported mint")]
    InvalidMint,
    /// The live deposit is above what the caller agreed to pay.
    #[msg("Deposit exceeds the caller's cap")]
    DepositTooHigh,
    /// An agent covers locations in one region only.
    #[msg("Agent is registered in a different region")]
    WrongRegion,
    #[msg("Agent already covers this location")]
    AlreadyInLocation,
    #[msg("Agent does not cover this location")]
    NotInLocation,
    /// The agent registry entry caps how many locations one agent covers.
    #[msg("Too many locations for one agent")]
    TooManyLocations,
    /// The regions program bounds postcodes; the entry's space relies on it.
    #[msg("Postcode is too long")]
    PostcodeTooLong,
    /// Leaving a location requires no properties assigned there.
    #[msg("Agent still manages properties in this location")]
    AgentStillAssigned,
}
