use anchor_lang::prelude::*;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";

#[constant]
pub const VAULT_SEED: &[u8] = b"vault";

#[constant]
pub const LAWYER_SEED: &[u8] = b"lawyer";

#[constant]
pub const PROPERTY_SEED: &[u8] = b"property";

#[constant]
pub const LISTING_SEED: &[u8] = b"listing";

#[constant]
pub const SHARE_MINT_SEED: &[u8] = b"share-mint";

#[constant]
pub const MINT_AUTH_SEED: &[u8] = b"mint-auth";

#[constant]
pub const PROPERTY_VAULT_SEED: &[u8] = b"property-vault";

#[constant]
pub const SHARE_SEED: &[u8] = b"share";

#[constant]
pub const POSITION_SEED: &[u8] = b"position";

#[constant]
pub const LISTING_VAULT_SEED: &[u8] = b"listing-vault";

#[constant]
pub const LAWYER_VOTE_SEED: &[u8] = b"lawyer-vote";

#[constant]
pub const LAWYER_CANDIDATE_SEED: &[u8] = b"lawyer-candidate";

#[constant]
pub const RESERVATION_SEED: &[u8] = b"reservation";

#[constant]
pub const CPI_AUTH_SEED: &[u8] = b"cpi-auth";

/// The property program, the only caller allowed on the share-lock surface.
/// `anchor keys sync` does not touch this, so update it by hand whenever the
/// property program id rotates.
pub const PROPERTY_PROGRAM: Pubkey = pubkey!("deCp9srk9C6P4BXJaFpjR5H6Jsm6DCq8AL2kk338dVq");

/// Seed of the property program's income ledger, mirrored here so the
/// settlement CPI can pin the account it inspects.
pub const INCOME_SEED: &[u8] = b"income";

#[constant]
pub const SHARE_LISTING_SEED: &[u8] = b"share-listing";
