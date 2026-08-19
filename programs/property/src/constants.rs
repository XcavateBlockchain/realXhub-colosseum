use anchor_lang::prelude::*;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";

#[constant]
pub const VAULT_SEED: &[u8] = b"vault";

#[constant]
pub const AGENT_SEED: &[u8] = b"agent";

#[constant]
pub const LETTING_SEED: &[u8] = b"letting";

#[constant]
pub const AGENT_CANDIDATE_SEED: &[u8] = b"agent-candidate";

#[constant]
pub const AGENT_VOTE_SEED: &[u8] = b"agent-vote";

#[constant]
pub const RESIGNATION_SEED: &[u8] = b"resignation";

/// Signer PDA for CPIs into the marketplace; carries no account data.
#[constant]
pub const CPI_AUTH_SEED: &[u8] = b"cpi-auth";

#[constant]
pub const INCOME_SEED: &[u8] = b"income";

#[constant]
pub const INCOME_VAULT_SEED: &[u8] = b"income-vault";

#[constant]
pub const CHECKPOINT_SEED: &[u8] = b"checkpoint";

#[constant]
pub const PROPOSAL_SEED: &[u8] = b"gov-proposal";

#[constant]
pub const PROPOSAL_VOTE_SEED: &[u8] = b"gov-proposal-vote";

#[constant]
pub const CHALLENGE_SEED: &[u8] = b"gov-challenge";

#[constant]
pub const CHALLENGE_VOTE_SEED: &[u8] = b"gov-challenge-vote";
