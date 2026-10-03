use anchor_lang::prelude::*;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";
#[constant]
pub const HUB_SEED: &[u8] = b"hub";
#[constant]
pub const HUB_MINT_SEED: &[u8] = b"hub_mint";
#[constant]
pub const TOKEN_VAULT_SEED: &[u8] = b"token_vault";
#[constant]
pub const PAYMENT_VAULT_SEED: &[u8] = b"payment_vault";
#[constant]
pub const BOND_VAULT_SEED: &[u8] = b"bond_vault";
#[constant]
pub const POSITION_SEED: &[u8] = b"position";

#[constant]
pub const RESERVATION_SALE_SEED: &[u8] = b"reservation_sale";
#[constant]
pub const HUB_RESERVATION_SEED: &[u8] = b"hub_reservation";
#[constant]
pub const PAYMENT_RESERVATION_SEED: &[u8] = b"payment_reservation";

#[constant]
pub const RESERVATION_CLAIM_SEED: &[u8] = b"reservation_claim";

#[constant]
pub const FUNDING_SEED: &[u8] = b"funding";

/// A fully reserved hub gives every buyer three complete days to pay.
pub const CLAIM_WINDOW_SECONDS: i64 = 3 * 24 * 60 * 60;

#[constant]
pub const MILESTONE_POLICY_SEED: &[u8] = b"milestone_policy";
#[constant]
pub const MILESTONE_SEED: &[u8] = b"milestone";

/// Evidence must receive approval within 60 days of the first tranche.
pub const MILESTONE_WINDOW_SECONDS: i64 = 60 * 24 * 60 * 60;
pub const PROBABILITY_SCALE: u16 = 10_000;

#[constant]
pub const DEFAULT_SEED: &[u8] = b"default";
#[constant]
pub const DEFAULT_REDEMPTION_SEED: &[u8] = b"default_redemption";
