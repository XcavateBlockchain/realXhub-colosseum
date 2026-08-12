use anchor_lang::prelude::*;

pub use regions::state::POSTCODE_MAX_LEN;

/// One agent covers at most this many locations; each one carries its own
/// deposit, so the bound also caps what a single registry entry can hold.
pub const MAX_AGENT_LOCATIONS: usize = 10;

#[account]
#[derive(InitSpace)]
pub struct Config {
    /// Authority allowed to update parameters.
    pub authority: Pubkey,
    /// Proposed replacement authority; takes over via `accept_authority`.
    /// Two-step so a typo'd address can't brick parameter management.
    pub pending_authority: Option<Pubkey>,
    /// The XCAV mint agent deposits are paid in.
    pub xcav_mint: Pubkey,
    /// Owner of the shared protocol treasury (a multisig on mainnet).
    /// Slashes are paid to this key's token accounts; the program never
    /// holds them.
    pub treasury: Pubkey,
    /// The sponsor wallet that fronts account rent for holders. Sponsored
    /// closes send their lamports here, not to the holder.
    pub rent_collector: Pubkey,
    /// XCAV an agent locks per location they register in.
    pub agent_deposit: u64,
    /// Seconds an agent election stays open once the first candidacy claims.
    pub agent_voting_time: i64,
    /// Share of a property's supply that must vote for an agent election to
    /// be valid, in basis points.
    pub min_voting_quorum_bps: u16,
    /// Seconds between an agent's resignation notice and the seat opening.
    pub agent_notice_period: i64,
    pub bump: u8,
}

/// Most agents that can stand in one election round. Bounds the account list
/// `finalize_agent_election` walks to find the plurality winner.
pub const MAX_AGENT_CANDIDATES: u32 = 5;

/// The running agent election. Any eligible agent may stand; the first
/// candidacy opens the voting window and shareholders vote among the
/// candidates. A round that elects nobody simply reopens: rounds repeat
/// until an agent is engaged. One round at a time per property.
#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, PartialEq, Eq, Debug, Default,
)]
pub struct AgentElection {
    /// When voting closes; zero while no round is running.
    pub expiry: i64,
    /// Candidates standing in the current round.
    pub candidate_count: u32,
    /// Round number, monotonic per property. Candidacies and vote records
    /// are keyed by it, so nothing stale can count toward a later round.
    pub round: u64,
    /// Quorum at the moment the round opened, so a config change can't move
    /// the goalposts mid-vote.
    pub quorum_bps: u16,
}

/// A property's letting seat: who manages it and the election that fills the
/// seat. Created by the first candidacy and kept for the property's life.
#[account]
#[derive(InitSpace)]
pub struct PropertyLetting {
    pub asset_id: u64,
    /// The assigned agent; default while the seat is vacant.
    pub agent: Pubkey,
    pub election: AgentElection,
    /// The wallet that fronted the account's rent.
    pub rent_payer: Pubkey,
    pub bump: u8,
}

/// One agent standing in one election round; carries their own tally.
#[account]
#[derive(InitSpace)]
pub struct AgentCandidacy {
    pub asset_id: u64,
    /// The election round the candidacy belongs to.
    pub round: u64,
    pub agent: Pubkey,
    /// Share-weighted votes cast for this candidate.
    pub vote_power: u32,
    /// The wallet that fronted the account's rent; refunded at close.
    pub rent_payer: Pubkey,
    pub bump: u8,
}

/// One investor's vote in one agent election round. The voting shares stay
/// locked in the marketplace ShareHolding until the record is closed again
/// by `unlock_agent_votes`.
#[account]
#[derive(InitSpace)]
pub struct AgentVote {
    pub asset_id: u64,
    /// The election round the vote belongs to.
    pub round: u64,
    pub voter: Pubkey,
    /// The candidate voted for.
    pub choice: Pubkey,
    /// The shares this vote locked and counts for.
    pub power: u32,
    /// The wallet that fronted the record's rent; refunded at close.
    pub rent_payer: Pubkey,
    pub bump: u8,
}

/// An assigned agent's notice that they are stepping down. The seat opens
/// once the notice period has run, via the `finalize_resignation` crank.
#[account]
#[derive(InitSpace)]
pub struct ResignationNotice {
    pub asset_id: u64,
    /// The resigning agent, recorded so the crank can release their
    /// assignment even if the seat's state moves on.
    pub agent: Pubkey,
    /// When the resignation takes effect.
    pub due_ts: i64,
    /// The wallet that fronted the account's rent; refunded at close.
    pub rent_payer: Pubkey,
    pub bump: u8,
}

/// A letting agent's registry entry, one per wallet. Agents work one region
/// and any number of its locations up to the cap; each location holds its
/// own deposit and counts the properties assigned there.
#[account]
#[derive(InitSpace)]
pub struct LettingAgent {
    pub wallet: Pubkey,
    /// The one region this agent covers.
    pub region_id: u16,
    #[max_len(MAX_AGENT_LOCATIONS)]
    pub locations: Vec<AgentLocation>,
    /// Who funded the account's rent and gets it back at close.
    pub rent_payer: Pubkey,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, PartialEq, Eq, Debug)]
pub struct AgentLocation {
    #[max_len(POSTCODE_MAX_LEN)]
    pub postcode: Vec<u8>,
    /// Properties currently assigned to the agent in this location. Leaving
    /// the location requires zero.
    pub assigned_count: u32,
    /// The deposit locked when this location was registered, recorded so a
    /// later config change can't reprice the refund.
    pub deposit: u64,
}
