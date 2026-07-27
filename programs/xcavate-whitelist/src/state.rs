use anchor_lang::prelude::*;

/// All roles recognised across the realXmarket protocol.
///
/// A role is app-level authorization, separate from KYC/compliance. Compliance
/// lives in [`AccessPermission`], and will eventually be driven by SAS
/// attestations rather than a manually set flag.
///
/// Do not reorder the variants: the serialized `RoleAccount.role` stores the
/// variant index, so a reorder reinterprets every existing assignment.
#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Manages a region: claims the operator seat, registers locations, sets
    /// listing duration and tax.
    RegionalOperator,
    /// Buys, claims, relists and votes on fractional property shares.
    RealEstateInvestor,
    /// Lists properties for fractional sale.
    RealEstateDeveloper,
    /// Represents the developer or SPV side of a property sale's legal process.
    Lawyer,
    /// Manages let properties and distributes rental income to share holders.
    LettingAgent,
    /// Confirms that the SPV for a sold-out property has been created.
    SpvConfirmation,
}

impl Role {
    /// Stable one-byte tag for PDA seeds. Explicit on purpose so the derivation
    /// doesn't shift if the enum is ever reordered.
    pub fn seed_byte(&self) -> u8 {
        match self {
            Role::RegionalOperator => 0,
            Role::RealEstateInvestor => 1,
            Role::RealEstateDeveloper => 2,
            Role::Lawyer => 3,
            Role::LettingAgent => 4,
            Role::SpvConfirmation => 5,
        }
    }
}

/// Compliance status for a (user, role) assignment, set by an admin after
/// off-chain KYC/AML. Only the marketplace's investor-fund calls (listing,
/// buying, offers, share transfers, lawyer case work) require this flag on
/// top of the role; every other instruction checks role possession alone.
/// Later it'll be driven by a SAS attestation instead of a manual toggle.
///
/// Do not reorder the variants: the serialized `RoleAccount.permission`
/// stores the variant index, so a reorder flips every stored status.
#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccessPermission {
    /// Passed KYC/AML, so role-specific actions are allowed.
    Compliant,
    /// Revoked, so role-specific actions are blocked.
    Revoked,
}

/// Singleton config holding the sudo authority that manages admins.
#[account]
#[derive(InitSpace)]
pub struct Config {
    /// Sudo authority allowed to add/remove admins.
    pub authority: Pubkey,
    /// Proposed replacement authority; takes over via `accept_authority`.
    /// Two-step so a typo'd address can't brick admin management.
    pub pending_authority: Option<Pubkey>,
    pub bump: u8,
}

/// Marks an address as a whitelist admin.
#[account]
#[derive(InitSpace)]
pub struct Admin {
    pub admin: Pubkey,
    pub bump: u8,
}

/// One (user, role) assignment together with its compliance status.
#[account]
#[derive(InitSpace)]
pub struct RoleAccount {
    pub user: Pubkey,
    pub role: Role,
    pub permission: AccessPermission,
    /// Who paid the account's rent (the assigning admin); every teardown
    /// refunds them, whoever triggers it.
    pub rent_payer: Pubkey,
    pub bump: u8,
}

impl RoleAccount {
    /// Whether this assignment is currently active (KYC-compliant).
    pub fn is_compliant(&self) -> bool {
        self.permission == AccessPermission::Compliant
    }
}
