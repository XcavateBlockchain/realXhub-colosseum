use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::token::spl_token;
use anchor_spl::token_2022::spl_token_2022::{
    extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions},
    state::Mint as MintState,
};

use crate::error::MarketplaceError;

/// Reject mints the vault accounting cannot support: an owner that isn't a
/// token program at all, a base-mint `freeze_authority` (whoever holds it
/// could lock the vault, and every refund with it), or a Token-2022 extension
/// that lets a third party move, block, or hide vaulted funds. A live
/// `mint_authority` stays allowed on purpose: the operator bond tracks the
/// XCAV supply, and changing that supply is a protocol-held lever. Returns
/// the mint's decimals for callers that bound them.
pub fn require_supported_mint(mint: &AccountInfo) -> Result<u8> {
    let data = mint.try_borrow_data()?;
    if *mint.owner == spl_token::ID {
        let state = spl_token::state::Mint::unpack(&data)?;
        require!(
            state.freeze_authority.is_none(),
            MarketplaceError::UnsupportedMintAuthority
        );
        return Ok(state.decimals);
    }
    require!(
        *mint.owner == anchor_spl::token_2022::ID,
        MarketplaceError::InvalidMint
    );
    let state = StateWithExtensions::<MintState>::unpack(&data)?;
    require!(
        state.base.freeze_authority.is_none(),
        MarketplaceError::UnsupportedMintAuthority
    );
    for extension in state.get_extension_types()? {
        match extension {
            ExtensionType::TransferFeeConfig
            | ExtensionType::MintCloseAuthority
            | ExtensionType::DefaultAccountState
            | ExtensionType::NonTransferable
            | ExtensionType::PermanentDelegate
            | ExtensionType::TransferHook
            | ExtensionType::Pausable
            | ExtensionType::ConfidentialTransferMint
            | ExtensionType::ConfidentialMintBurn => {
                return err!(MarketplaceError::UnsupportedMintExtension);
            }
            _ => {}
        }
    }
    Ok(state.base.decimals)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classic_mint_data() -> Vec<u8> {
        let mut data = vec![0u8; 82];
        data[45] = 1; // is_initialized
        data
    }

    fn t22_mint_data(extension_type: u16, len: u16) -> Vec<u8> {
        // Base mint (82 bytes) padded to the account type offset (165), the
        // mint tag, then a single TLV entry header plus a zeroed payload.
        let mut data = vec![0u8; 166 + 4 + len as usize];
        data[45] = 1; // is_initialized
        data[165] = 1; // account type: mint
        data[166..168].copy_from_slice(&extension_type.to_le_bytes());
        data[168..170].copy_from_slice(&len.to_le_bytes());
        data
    }

    // The base mint's freeze authority is a COption tag at offset 46.
    fn with_lock_authority(mut data: Vec<u8>) -> Vec<u8> {
        data[46] = 1;
        data
    }

    fn check(owner: Pubkey, mut data: Vec<u8>) -> Result<u8> {
        let key = Pubkey::new_unique();
        let mut lamports = 0u64;
        let info = AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        require_supported_mint(&info)
    }

    #[test]
    fn classic_mint_passes() {
        assert!(check(spl_token::ID, classic_mint_data()).is_ok());
    }

    #[test]
    fn non_token_owner_rejected() {
        // Valid mint bytes under the wrong owner must not parse as a mint.
        let owner = Pubkey::new_unique();
        assert!(check(owner, t22_mint_data(18, 64)).is_err());
    }

    #[test]
    fn classic_mint_with_lock_authority_rejected() {
        assert!(check(spl_token::ID, with_lock_authority(classic_mint_data())).is_err());
    }

    #[test]
    fn t22_mint_with_lock_authority_rejected() {
        // Metadata pointer (18) is harmless, so the rejection is the authority.
        let data = with_lock_authority(t22_mint_data(18, 64));
        assert!(check(anchor_spl::token_2022::ID, data).is_err());
    }

    #[test]
    fn fee_bearing_mint_rejected() {
        // Extension type 1 is the transfer fee config.
        assert!(check(anchor_spl::token_2022::ID, t22_mint_data(1, 108)).is_err());
    }

    #[test]
    fn permanent_delegate_rejected() {
        // Extension type 12 is the permanent delegate.
        assert!(check(anchor_spl::token_2022::ID, t22_mint_data(12, 32)).is_err());
    }

    #[test]
    fn pausable_mint_rejected() {
        // Extension type 26 is the pausable config.
        assert!(check(anchor_spl::token_2022::ID, t22_mint_data(26, 33)).is_err());
    }

    #[test]
    fn metadata_pointer_allowed() {
        // Extension type 18 is the metadata pointer, which is harmless here.
        assert!(check(anchor_spl::token_2022::ID, t22_mint_data(18, 64)).is_ok());
    }
}
