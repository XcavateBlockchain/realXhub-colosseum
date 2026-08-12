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
    /// Elections only run on settled properties.
    #[msg("Property is not finalized")]
    PropertyNotFinalized,
    #[msg("The property already has an assigned agent")]
    SeatTaken,
    #[msg("No election is running for this property")]
    NoElectionRunning,
    /// Candidacies and votes must land inside the round's window.
    #[msg("The voting window has closed")]
    VotingClosed,
    #[msg("The voting window is still open")]
    VotingStillOngoing,
    /// Rounds are numbered; a claim must name the running round, or the next
    /// one to open a fresh window.
    #[msg("Wrong election round")]
    WrongElectionRound,
    #[msg("Too many candidates in this round")]
    TooManyCandidates,
    /// A candidacy account doesn't belong to this property and round.
    #[msg("Candidacy does not match the election")]
    CandidacyMismatch,
    #[msg("Vote amount must be greater than zero")]
    InvalidVoteAmount,
    /// The finalizer needs the winner's registry entry to inspect.
    #[msg("Wrong or missing agent registry entry")]
    WrongAgent,
    #[msg("Wrong rent payer")]
    WrongRentPayer,
    #[msg("Signer is not the property's assigned agent")]
    NotAssignedAgent,
    #[msg("The resignation notice period is still running")]
    NoticePeriodRunning,
}
