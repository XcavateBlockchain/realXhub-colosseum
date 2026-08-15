pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("7TrzjKpdrEhnfhxuw8tWdH1sjxadazscsG5HXCDPLmaY");

/// Roles and compliance registry for the realXmarket protocol.
///
/// Tracks which addresses hold which roles and whether each assignment is
/// KYC-compliant. Other programs gate actions by loading the `RoleAccount`
/// PDA (`["role", user, role.seed_byte()]`), which requires the role to be
/// assigned.
#[program]
pub mod xcavate_whitelist {
    use super::*;

    /// Initialize the singleton config; sets sudo authority to the signer.
    pub fn initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
        initialize::handler(ctx)
    }

    /// Propose a new sudo authority (two-step handover). Current-authority-only.
    pub fn update_authority(ctx: Context<UpdateAuthority>, new_authority: Pubkey) -> Result<()> {
        initialize::update_authority_handler(ctx, new_authority)
    }

    /// Complete the authority handover. Signed by the pending authority.
    pub fn accept_authority(ctx: Context<AcceptAuthority>) -> Result<()> {
        initialize::accept_authority_handler(ctx)
    }

    /// Register a whitelist admin. Sudo-only.
    pub fn add_admin(ctx: Context<AddAdmin>) -> Result<()> {
        admin::add_admin_handler(ctx)
    }

    /// Remove a whitelist admin. Sudo-only.
    pub fn remove_admin(ctx: Context<RemoveAdmin>, admin_key: Pubkey) -> Result<()> {
        admin::remove_admin_handler(ctx, admin_key)
    }

    /// Assign a role to a user (default Compliant). Admin-only.
    pub fn assign_role(ctx: Context<AssignRole>, role: Role) -> Result<()> {
        role::assign_role_handler(ctx, role)
    }

    /// Remove a role from a user. Admin-only.
    pub fn remove_role(ctx: Context<RemoveRole>, role: Role) -> Result<()> {
        role::remove_role_handler(ctx, role)
    }

    /// Give up one's own role. Signed by the role holder.
    pub fn renounce_role(ctx: Context<RenounceRole>, role: Role) -> Result<()> {
        role::renounce_role_handler(ctx, role)
    }

    /// Update a user's compliance status for a role. Admin-only.
    pub fn set_permission(
        ctx: Context<SetPermission>,
        role: Role,
        permission: AccessPermission,
    ) -> Result<()> {
        role::set_permission_handler(ctx, role, permission)
    }
}
