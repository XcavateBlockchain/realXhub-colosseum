//! Shared test scaffolding for the marketplace: PDA and token helpers,
//! instruction builders, the send/assert helpers (`ok`, `fails_with`), and
//! `setup`. Deposits are XCAV (a classic SPL token) held in the program vault;
//! LiteSVM loads the SPL Token program by default, and the XCAV mint and each
//! participant's token account are seeded directly with `set_account`. Each
//! test file pulls this in with `mod common; use common::*;`.
//!
//! Each test file is its own binary that uses a subset of this, so unused
//! helpers are expected.
#![allow(dead_code, unused_imports)]

pub use anchor_lang::prelude::Pubkey;
pub use anchor_lang::solana_program::clock::Clock;
pub use anchor_lang::AccountDeserialize;
pub use litesvm::LiteSVM;
pub use marketplace::state::Config as MarketplaceConfig;
pub use solana_keypair::Keypair;
pub use solana_signer::Signer;
pub use xcavate_whitelist::state::Role;

use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::program_option::COption;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token::state::{Account as SplAccount, AccountState, Mint as SplMint};
use anchor_spl::token::ID as TOKEN_PROGRAM_ID;
use litesvm::types::{FailedTransactionMetadata, TransactionMetadata};
use solana_account::Account;
use solana_message::{Message, VersionedMessage};
use solana_transaction::versioned::VersionedTransaction;

use anchor_lang::{AnchorSerialize, Discriminator};
use marketplace::instructions::ConfigParams;
use marketplace::{CONFIG_SEED, LAWYER_SEED, LISTING_SEED, PROPERTY_SEED, VAULT_SEED};

pub const SYS: Pubkey = anchor_lang::system_program::ID;
pub const DECIMALS: u8 = 9;
pub const FUND_XCAV: u64 = 100_000_000_000;
pub const LISTING_DEPOSIT: u64 = 1_000_000_000;
pub const LAWYER_DEPOSIT: u64 = 500_000_000;
pub const SHARE_PRICE: u64 = 5_000_000_000;
pub const SHARE_AMOUNT: u32 = 100;
/// Matches the `listing_duration` that `seed_region` writes.
pub const LISTING_DURATION: i64 = 100_000;
pub const POSTCODE: &[u8] = b"SW1A1AA";

// --- ids / PDAs ---

pub fn mid() -> Pubkey {
    marketplace::id()
}
pub fn roles_id() -> Pubkey {
    xcavate_whitelist::id()
}

pub fn marketplace_config() -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED], &mid()).0
}
pub fn vault() -> Pubkey {
    Pubkey::find_program_address(&[VAULT_SEED], &mid()).0
}

pub fn lawyer_pda(wallet: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[LAWYER_SEED, wallet.as_ref()], &mid()).0
}
pub fn region_pda(region_id: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[regions::REGION_SEED, &region_id.to_le_bytes()],
        &regions::id(),
    )
    .0
}

pub fn property_pda(asset_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[PROPERTY_SEED, &asset_id.to_le_bytes()], &mid()).0
}
pub fn listing_pda(listing_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[LISTING_SEED, &listing_id.to_le_bytes()], &mid()).0
}
pub fn location_pda(region_id: u16, postcode: &[u8]) -> Pubkey {
    Pubkey::find_program_address(
        &[regions::LOCATION_SEED, &region_id.to_le_bytes(), postcode],
        &regions::id(),
    )
    .0
}

pub fn roles_config() -> Pubkey {
    Pubkey::find_program_address(&[xcavate_whitelist::CONFIG_SEED], &roles_id()).0
}
pub fn admin_pda(who: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[xcavate_whitelist::ADMIN_SEED, who.as_ref()], &roles_id()).0
}
pub fn role_pda(user: &Pubkey, role: Role) -> Pubkey {
    Pubkey::find_program_address(
        &[
            xcavate_whitelist::ROLE_SEED,
            user.as_ref(),
            &[role.seed_byte()],
        ],
        &roles_id(),
    )
    .0
}

// --- XCAV mint / token accounts (seeded directly) ---

/// Fixed address for the test XCAV mint.
pub fn xcav_mint() -> Pubkey {
    Pubkey::new_from_array([7u8; 32])
}

/// Fixed address for the test tGBP payment mint.
pub fn tgbp_mint() -> Pubkey {
    Pubkey::new_from_array([6u8; 32])
}

/// Deterministic XCAV token account for an owner. Not a real ATA; the program
/// only checks the mint and authority, so any token account works.
pub fn token_acc(owner: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"xcav_token", owner.as_ref()], &mid()).0
}

pub fn set_mint(svm: &mut LiteSVM) {
    let mint = SplMint {
        mint_authority: COption::None,
        supply: 1_000_000_000_000,
        decimals: DECIMALS,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    let mut data = vec![0u8; SplMint::LEN];
    mint.pack_into_slice(&mut data);
    svm.set_account(
        xcav_mint(),
        Account {
            lamports: 100_000_000,
            data,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

pub fn set_token_account(svm: &mut LiteSVM, address: Pubkey, owner: &Pubkey, amount: u64) {
    let acc = SplAccount {
        mint: xcav_mint(),
        owner: *owner,
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    let mut data = vec![0u8; SplAccount::LEN];
    acc.pack_into_slice(&mut data);
    svm.set_account(
        address,
        Account {
            lamports: 100_000_000,
            data,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

pub fn give_xcav(svm: &mut LiteSVM, owner: &Pubkey, amount: u64) {
    set_token_account(svm, token_acc(owner), owner, amount);
}

pub fn xcav_balance(svm: &LiteSVM, owner: &Pubkey) -> u64 {
    let acc = svm.get_account(&token_acc(owner)).unwrap();
    SplAccount::unpack(&acc.data).unwrap().amount
}

pub fn vault_balance(svm: &LiteSVM) -> u64 {
    let acc = svm.get_account(&vault()).unwrap();
    SplAccount::unpack(&acc.data).unwrap().amount
}

// --- send helpers ---

pub fn process(
    svm: &mut LiteSVM,
    ix: Instruction,
    payer: &Keypair,
    signers: &[&Keypair],
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

pub fn ok(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair, signers: &[&Keypair]) {
    if let Err(failed) = process(svm, ix, payer, signers) {
        panic!("expected success, failed with: {:?}", failed.err);
    }
}

pub fn fails_with(
    svm: &mut LiteSVM,
    ix: Instruction,
    payer: &Keypair,
    signers: &[&Keypair],
    expected: &str,
) {
    match process(svm, ix, payer, signers) {
        Ok(_) => panic!("expected failure `{expected}`, but it succeeded"),
        Err(failed) => {
            let detail = format!("{:?}\n{}", failed.err, failed.meta.logs.join("\n"));
            assert!(
                detail.contains(expected),
                "expected `{expected}`, got:\n{detail}"
            );
        }
    }
}

/// A SOL-funded keypair (for fees + account rent).
pub fn funded(svm: &mut LiteSVM) -> Keypair {
    let kp = Keypair::new();
    svm.airdrop(&kp.pubkey(), 100_000_000_000).unwrap();
    kp
}

/// A SOL-funded keypair that also holds XCAV.
pub fn actor(svm: &mut LiteSVM) -> Keypair {
    let kp = funded(svm);
    give_xcav(svm, &kp.pubkey(), FUND_XCAV);
    kp
}

// --- upgrade authority plumbing ---

// The programdata account the upgradeable loader keeps beside each program.
pub fn program_data_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[program_id.as_ref()],
        &anchor_lang::solana_program::bpf_loader_upgradeable::ID,
    )
    .0
}

// Point a deployed program's upgrade authority at `authority` so the
// authority-bound initialize passes. The loader metadata is 4 bytes of enum
// tag, 8 of slot, then an optional pubkey.
pub fn bind_upgrade_authority(svm: &mut LiteSVM, program_id: &Pubkey, authority: &Pubkey) {
    let pd = program_data_pda(program_id);
    let mut acc = svm.get_account(&pd).unwrap();
    acc.data[12] = 1;
    acc.data[13..45].copy_from_slice(authority.as_ref());
    svm.set_account(pd, acc).unwrap();
}

// --- roles instruction builders ---

pub fn roles_init_ix(authority: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        roles_id(),
        &xcavate_whitelist::instruction::InitializeConfig {}.data(),
        xcavate_whitelist::accounts::InitializeConfig {
            authority: *authority,
            program: roles_id(),
            program_data: program_data_pda(&roles_id()),
            config: roles_config(),
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

pub fn roles_add_admin_ix(authority: &Pubkey, new_admin: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        roles_id(),
        &xcavate_whitelist::instruction::AddAdmin {}.data(),
        xcavate_whitelist::accounts::AddAdmin {
            authority: *authority,
            config: roles_config(),
            new_admin: *new_admin,
            admin: admin_pda(new_admin),
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

pub fn roles_assign_ix(admin: &Pubkey, user: &Pubkey, role: Role) -> Instruction {
    Instruction::new_with_bytes(
        roles_id(),
        &xcavate_whitelist::instruction::AssignRole { role }.data(),
        xcavate_whitelist::accounts::AssignRole {
            admin_signer: *admin,
            admin: admin_pda(admin),
            user: *user,
            role_account: role_pda(user, role),
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

// --- marketplace instruction builders ---

pub fn default_params() -> ConfigParams {
    ConfigParams {
        treasury: Pubkey::new_from_array([9u8; 32]),
        rent_collector: Pubkey::new_from_array([8u8; 32]),
        accepted_payment_mints: vec![tgbp_mint()],
        listing_deposit: LISTING_DEPOSIT,
        lawyer_deposit: LAWYER_DEPOSIT,
        min_property_shares: 1,
        max_property_shares: 100,
        marketplace_fee_bps: 100,
        investor_fee_bps: 100,
        max_ownership_bps: 5_000,
        legal_process_time: 100_000,
        lawyer_voting_time: 10_000,
        min_voting_quorum_bps: 2_500,
    }
}

pub fn init_ix(authority: &Pubkey) -> Instruction {
    init_ix_with(authority, default_params())
}

pub fn init_ix_with(authority: &Pubkey, params: ConfigParams) -> Instruction {
    Instruction::new_with_bytes(
        mid(),
        &marketplace::instruction::InitializeConfig { params }.data(),
        marketplace::accounts::InitializeConfig {
            authority: *authority,
            program: mid(),
            program_data: program_data_pda(&mid()),
            config: marketplace_config(),
            xcav_mint: xcav_mint(),
            vault: vault(),
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

pub fn update_config_ix(authority: &Pubkey, params: ConfigParams) -> Instruction {
    Instruction::new_with_bytes(
        mid(),
        &marketplace::instruction::UpdateConfig { params }.data(),
        marketplace::accounts::UpdateConfig {
            authority: *authority,
            config: marketplace_config(),
        }
        .to_account_metas(None),
    )
}

pub fn update_authority_ix(authority: &Pubkey, new_authority: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        mid(),
        &marketplace::instruction::UpdateAuthority {
            new_authority: *new_authority,
        }
        .data(),
        marketplace::accounts::UpdateAuthority {
            authority: *authority,
            config: marketplace_config(),
        }
        .to_account_metas(None),
    )
}

pub fn accept_authority_ix(new_authority: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        mid(),
        &marketplace::instruction::AcceptAuthority {}.data(),
        marketplace::accounts::AcceptAuthority {
            new_authority: *new_authority,
            config: marketplace_config(),
        }
        .to_account_metas(None),
    )
}

/// Write a created `Region` account at its canonical PDA, exactly as the
/// regions program would leave it. Registering a lawyer only needs the account
/// to exist, so tests skip the whole proposal/vote/claim dance.
pub fn seed_region(svm: &mut LiteSVM, region_id: u16, owner: &Pubkey) {
    let (address, bump) = Pubkey::find_program_address(
        &[regions::REGION_SEED, &region_id.to_le_bytes()],
        &regions::id(),
    );
    let region = regions::state::Region {
        region_id,
        owner: *owner,
        collateral: 0,
        next_owner_change: i64::MAX,
        listing_duration: 100_000,
        tax_bps: 300,
        location_count: 0,
        bump,
    };
    let mut data = regions::state::Region::DISCRIMINATOR.to_vec();
    region.serialize(&mut data).unwrap();
    svm.set_account(
        address,
        Account {
            lamports: 100_000_000,
            data,
            owner: regions::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

/// Write a registered `Location` account at its canonical PDA, exactly as the
/// regions program would leave it. Listing only needs the account to exist.
pub fn seed_location(svm: &mut LiteSVM, region_id: u16, postcode: &[u8]) {
    let (address, bump) = Pubkey::find_program_address(
        &[regions::LOCATION_SEED, &region_id.to_le_bytes(), postcode],
        &regions::id(),
    );
    let location = regions::state::Location {
        region_id,
        postcode: postcode.to_vec(),
        bump,
    };
    let mut data = regions::state::Location::DISCRIMINATOR.to_vec();
    location.serialize(&mut data).unwrap();
    svm.set_account(
        address,
        Account {
            lamports: 100_000_000,
            data,
            owner: regions::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

/// A SOL-funded, XCAV-holding keypair with the RealEstateDeveloper role.
pub fn new_developer(svm: &mut LiteSVM, admin: &Keypair) -> Keypair {
    let kp = actor(svm);
    ok(
        svm,
        roles_assign_ix(&admin.pubkey(), &kp.pubkey(), Role::RealEstateDeveloper),
        admin,
        &[admin],
    );
    kp
}

pub fn list_property_ix(
    developer: &Pubkey,
    listing_id: u64,
    region_id: u16,
    postcode: &[u8],
    share_price: u64,
    share_amount: u32,
) -> Instruction {
    Instruction::new_with_bytes(
        mid(),
        &marketplace::instruction::ListProperty {
            region_id,
            postcode: postcode.to_vec(),
            share_price,
            share_amount,
            tax_paid_by_developer: false,
        }
        .data(),
        marketplace::accounts::ListProperty {
            developer: *developer,
            config: marketplace_config(),
            developer_role: role_pda(developer, Role::RealEstateDeveloper),
            region: region_pda(region_id),
            location: location_pda(region_id, postcode),
            property: property_pda(listing_id),
            listing: listing_pda(listing_id),
            xcav_mint: xcav_mint(),
            developer_token: token_acc(developer),
            vault: vault(),
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

/// List with the default region 1 / seeded postcode / default price and amount.
pub fn list_ix(developer: &Pubkey, listing_id: u64) -> Instruction {
    list_property_ix(
        developer,
        listing_id,
        1,
        POSTCODE,
        SHARE_PRICE,
        SHARE_AMOUNT,
    )
}

pub fn upgrade_ix(developer: &Pubkey, listing_id: u64, new_price: u64) -> Instruction {
    Instruction::new_with_bytes(
        mid(),
        &marketplace::instruction::UpgradeObject {
            listing_id,
            new_price,
        }
        .data(),
        marketplace::accounts::UpgradeObject {
            developer: *developer,
            developer_role: role_pda(developer, Role::RealEstateDeveloper),
            listing: listing_pda(listing_id),
        }
        .to_account_metas(None),
    )
}

pub fn listing_of(svm: &LiteSVM, listing_id: u64) -> marketplace::state::Listing {
    marketplace::state::Listing::try_deserialize(
        &mut &svm.get_account(&listing_pda(listing_id)).unwrap().data[..],
    )
    .unwrap()
}

pub fn property_of(svm: &LiteSVM, asset_id: u64) -> marketplace::state::PropertyAsset {
    marketplace::state::PropertyAsset::try_deserialize(
        &mut &svm.get_account(&property_pda(asset_id)).unwrap().data[..],
    )
    .unwrap()
}

pub fn register_lawyer_ix(lawyer: &Pubkey, region_id: u16) -> Instruction {
    Instruction::new_with_bytes(
        mid(),
        &marketplace::instruction::RegisterLawyer { region_id }.data(),
        marketplace::accounts::RegisterLawyer {
            lawyer: *lawyer,
            config: marketplace_config(),
            lawyer_role: role_pda(lawyer, Role::Lawyer),
            region: region_pda(region_id),
            lawyer_account: lawyer_pda(lawyer),
            xcav_mint: xcav_mint(),
            lawyer_token: token_acc(lawyer),
            vault: vault(),
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

pub fn unregister_lawyer_ix(lawyer: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        mid(),
        &marketplace::instruction::UnregisterLawyer {}.data(),
        marketplace::accounts::UnregisterLawyer {
            lawyer: *lawyer,
            config: marketplace_config(),
            lawyer_role: role_pda(lawyer, Role::Lawyer),
            lawyer_account: lawyer_pda(lawyer),
            xcav_mint: xcav_mint(),
            lawyer_token: token_acc(lawyer),
            vault: vault(),
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}

pub fn lawyer_of(svm: &LiteSVM, wallet: &Pubkey) -> marketplace::state::Lawyer {
    marketplace::state::Lawyer::try_deserialize(
        &mut &svm.get_account(&lawyer_pda(wallet)).unwrap().data[..],
    )
    .unwrap()
}

/// Overwrite a registered lawyer's active-case count. The instructions that
/// assign and close cases aren't built yet, so tests poke the field directly.
pub fn set_active_cases(svm: &mut LiteSVM, wallet: &Pubkey, active_cases: u32) {
    let mut lawyer = lawyer_of(svm, wallet);
    lawyer.active_cases = active_cases;
    let mut data = marketplace::state::Lawyer::DISCRIMINATOR.to_vec();
    lawyer.serialize(&mut data).unwrap();
    let mut acc = svm.get_account(&lawyer_pda(wallet)).unwrap();
    acc.data = data;
    svm.set_account(lawyer_pda(wallet), acc).unwrap();
}

/// A SOL-funded, XCAV-holding keypair with the Lawyer role.
pub fn new_lawyer(svm: &mut LiteSVM, admin: &Keypair) -> Keypair {
    let kp = actor(svm);
    ok(
        svm,
        roles_assign_ix(&admin.pubkey(), &kp.pubkey(), Role::Lawyer),
        admin,
        &[admin],
    );
    kp
}

/// Flips a role assignment's compliance flag through the whitelist program.
pub fn set_permission(
    svm: &mut LiteSVM,
    admin: &Keypair,
    user: &Pubkey,
    role: Role,
    compliant: bool,
) {
    let permission = if compliant {
        xcavate_whitelist::state::AccessPermission::Compliant
    } else {
        xcavate_whitelist::state::AccessPermission::Revoked
    };
    let ix = Instruction::new_with_bytes(
        roles_id(),
        &xcavate_whitelist::instruction::SetPermission { role, permission }.data(),
        xcavate_whitelist::accounts::SetPermission {
            admin_signer: admin.pubkey(),
            admin: admin_pda(&admin.pubkey()),
            user: *user,
            role_account: role_pda(user, role),
        }
        .to_account_metas(None),
    );
    ok(svm, ix, admin, &[admin]);
}

// --- setup ---

pub fn config_of(svm: &LiteSVM) -> MarketplaceConfig {
    MarketplaceConfig::try_deserialize(
        &mut &svm.get_account(&marketplace_config()).unwrap().data[..],
    )
    .unwrap()
}

/// Advance the clock by `secs` seconds.
pub fn warp(svm: &mut LiteSVM, secs: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += secs;
    svm.set_sysvar(&clock);
}

// Loads the roles and marketplace programs, seeds the XCAV mint, initializes
// both configs, and returns (svm, admin, authority). The admin can hand out
// roles.
pub fn setup() -> (LiteSVM, Keypair, Keypair) {
    let mut svm = LiteSVM::new();
    svm.add_program(
        roles_id(),
        include_bytes!("../../../../target/deploy/xcavate_whitelist.so"),
    )
    .unwrap();
    svm.add_program(
        mid(),
        include_bytes!("../../../../target/deploy/marketplace.so"),
    )
    .unwrap();
    set_mint(&mut svm);

    let authority = funded(&mut svm);
    bind_upgrade_authority(&mut svm, &roles_id(), &authority.pubkey());
    bind_upgrade_authority(&mut svm, &mid(), &authority.pubkey());
    ok(
        &mut svm,
        roles_init_ix(&authority.pubkey()),
        &authority,
        &[&authority],
    );

    let admin = funded(&mut svm);
    ok(
        &mut svm,
        roles_add_admin_ix(&authority.pubkey(), &admin.pubkey()),
        &authority,
        &[&authority],
    );

    ok(
        &mut svm,
        init_ix(&authority.pubkey()),
        &authority,
        &[&authority],
    );
    (svm, admin, authority)
}
