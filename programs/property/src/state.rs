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
