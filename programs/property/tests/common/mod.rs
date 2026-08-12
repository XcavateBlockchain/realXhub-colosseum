//! Shared test scaffolding for the property program: PDA and token helpers,
//! instruction builders, the send/assert helpers (`ok`, `fails_with`), and
//! `setup`. Deposits are XCAV (a classic SPL token) held in the program
//! vault; the XCAV mint and each participant's token account are seeded
//! directly with `set_account`. Each test file pulls this in with
//! `mod common; use common::*;`.
//!
//! Each test file is its own binary that uses a subset of this, so unused
//! helpers are expected.
#![allow(dead_code, unused_imports)]

pub use anchor_lang::prelude::Pubkey;
pub use anchor_lang::solana_program::clock::Clock;
pub use anchor_lang::AccountDeserialize;
pub use litesvm::LiteSVM;
pub use marketplace::state::{PropertyAsset, ShareHolding};
pub use property::state::{
    AgentCandidacy, AgentVote, Config as PropertyConfig, LettingAgent, PropertyLetting,
    ResignationNotice,
};
pub use solana_keypair::Keypair;
pub use solana_signer::Signer;
pub use xcavate_whitelist::state::Role;

use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::program_option::COption;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_lang::{AnchorSerialize, Discriminator};
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token::state::{Account as SplAccount, AccountState, Mint as SplMint};
use anchor_spl::token::ID as TOKEN_PROGRAM_ID;
use litesvm::types::{FailedTransactionMetadata, TransactionMetadata};
use anchor_lang::solana_program::instruction::AccountMeta;
use property::instructions::ConfigParams;
use property::{
    AGENT_CANDIDATE_SEED, AGENT_SEED, AGENT_VOTE_SEED, CONFIG_SEED, CPI_AUTH_SEED, LETTING_SEED,
    RESIGNATION_SEED, VAULT_SEED,
};
use solana_account::Account;
use solana_message::{Message, VersionedMessage};
use solana_transaction::versioned::VersionedTransaction;

pub const SYS: Pubkey = anchor_lang::system_program::ID;
pub const DECIMALS: u8 = 9;
pub const FUND_XCAV: u64 = 100_000_000_000;
pub const AGENT_DEPOSIT: u64 = 200_000_000;
pub const POSTCODE: &[u8] = b"SW1A1AA";
pub const POSTCODE_B: &[u8] = b"E20 2ST";
pub const VOTING_TIME: i64 = 3_600;
pub const QUORUM_BPS: u16 = 2_500;
pub const NOTICE_PERIOD: i64 = 86_400;
pub const SHARE_SUPPLY: u32 = 100;

// --- ids / PDAs ---

pub fn pid() -> Pubkey {
    property::id()
}
pub fn roles_id() -> Pubkey {
    xcavate_whitelist::id()
}

pub fn property_config() -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED], &pid()).0
}
pub fn vault() -> Pubkey {
    Pubkey::find_program_address(&[VAULT_SEED], &pid()).0
}
pub fn agent_pda(wallet: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[AGENT_SEED, wallet.as_ref()], &pid()).0
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
pub fn location_pda(region_id: u16, postcode: &[u8]) -> Pubkey {
    Pubkey::find_program_address(
        &[regions::LOCATION_SEED, &region_id.to_le_bytes(), postcode],
        &regions::id(),
    )
    .0
}

pub fn mid() -> Pubkey {
    marketplace::id()
}
pub fn letting_pda(asset_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[LETTING_SEED, &asset_id.to_le_bytes()], &pid()).0
}
pub fn candidacy_pda(asset_id: u64, round: u64, agent: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            AGENT_CANDIDATE_SEED,
            &asset_id.to_le_bytes(),
            &round.to_le_bytes(),
            agent.as_ref(),
        ],
        &pid(),
    )
    .0
}
pub fn agent_vote_pda(asset_id: u64, round: u64, voter: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            AGENT_VOTE_SEED,
            &asset_id.to_le_bytes(),
            &round.to_le_bytes(),
            voter.as_ref(),
        ],
        &pid(),
    )
    .0
}
pub fn resignation_pda(asset_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[RESIGNATION_SEED, &asset_id.to_le_bytes()], &pid()).0
}
pub fn cpi_auth() -> Pubkey {
    Pubkey::find_program_address(&[CPI_AUTH_SEED], &pid()).0
}
pub fn mkt_property_pda(asset_id: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[marketplace::PROPERTY_SEED, &asset_id.to_le_bytes()],
        &mid(),
    )
    .0
}
pub fn holding_pda(asset_id: u64, owner: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            marketplace::SHARE_SEED,
            &asset_id.to_le_bytes(),
            owner.as_ref(),
        ],
        &mid(),
    )
    .0
}

// --- token plumbing ---

pub fn xcav_mint() -> Pubkey {
    Pubkey::new_from_array([7u8; 32])
}

pub fn treasury() -> Pubkey {
    Pubkey::new_from_array([21u8; 32])
}

/// The sponsor wallet fronting holder rent. Deterministic, so
/// `default_params` can name it as the rent collector.
pub fn sponsor() -> Keypair {
    Keypair::new_from_array([42u8; 32])
}

/// Deterministic XCAV token account for an owner. Not a real ATA; the
/// program only checks the mint and authority, so any token account works.
pub fn token_acc(owner: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"xcav_token", owner.as_ref()], &pid()).0
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

// --- send/assert ---

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

pub fn roles_revoke_ix(admin: &Pubkey, user: &Pubkey, role: Role) -> Instruction {
    Instruction::new_with_bytes(
        roles_id(),
        &xcavate_whitelist::instruction::RemoveRole { role }.data(),
        xcavate_whitelist::accounts::RemoveRole {
            admin_signer: *admin,
            admin: admin_pda(admin),
            user: *user,
            rent_payer: *admin,
            role_account: role_pda(user, role),
        }
        .to_account_metas(None),
    )
}

// --- regions account seeding ---

/// Write a registered `Location` account at its canonical PDA, exactly as
/// the regions program would leave it. Registering an agent only needs the
/// account to exist, so tests skip the whole region lifecycle.
pub fn seed_location(svm: &mut LiteSVM, region_id: u16, postcode: &[u8]) {
    let (address, bump) = Pubkey::find_program_address(
        &[regions::LOCATION_SEED, &region_id.to_le_bytes(), postcode],
        &regions::id(),
    );
    let location = regions::state::Location {
        region_id,
        postcode: postcode.to_vec(),
        deposit: 50_000_000,
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

// --- marketplace account seeding ---

/// Write a finalized `PropertyAsset` at its canonical marketplace PDA. The
/// election only reads it, so tests skip the whole primary-sale lifecycle.
pub fn seed_property_asset(svm: &mut LiteSVM, asset_id: u64, region_id: u16, postcode: &[u8]) {
    let (address, bump) = Pubkey::find_program_address(
        &[marketplace::PROPERTY_SEED, &asset_id.to_le_bytes()],
        &mid(),
    );
    let property = PropertyAsset {
        asset_id,
        core_asset: Pubkey::new_unique(),
        share_mint: Pubkey::new_unique(),
        region_id,
        location: postcode.to_vec(),
        share_amount: SHARE_SUPPLY,
        spv_created: true,
        finalized: true,
        holder_count: 3,
        bump,
    };
    let mut data = PropertyAsset::DISCRIMINATOR.to_vec();
    property.serialize(&mut data).unwrap();
    svm.set_account(
        address,
        Account {
            lamports: 100_000_000,
            data,
            owner: mid(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

/// Mark the seeded property as not (or again) finalized.
pub fn set_property_finalized(svm: &mut LiteSVM, asset_id: u64, finalized: bool) {
    let address = mkt_property_pda(asset_id);
    let acc = svm.get_account(&address).unwrap();
    let mut property = PropertyAsset::try_deserialize(&mut acc.data.as_slice()).unwrap();
    property.finalized = finalized;
    let mut data = PropertyAsset::DISCRIMINATOR.to_vec();
    property.serialize(&mut data).unwrap();
    svm.set_account(address, Account { data, ..acc }).unwrap();
}

/// Write a holder's `ShareHolding` at its canonical marketplace PDA.
pub fn seed_holding(svm: &mut LiteSVM, asset_id: u64, owner: &Pubkey, amount: u32) {
    let (address, bump) = Pubkey::find_program_address(
        &[
            marketplace::SHARE_SEED,
            &asset_id.to_le_bytes(),
            owner.as_ref(),
        ],
        &mid(),
    );
    let holding = ShareHolding {
        asset_id,
        owner: *owner,
        amount,
        locked_amount: 0,
        bump,
    };
    let mut data = ShareHolding::DISCRIMINATOR.to_vec();
    holding.serialize(&mut data).unwrap();
    svm.set_account(
        address,
        Account {
            lamports: 100_000_000,
            data,
            owner: mid(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

// --- property instruction builders ---

pub fn default_params() -> ConfigParams {
    ConfigParams {
        treasury: treasury(),
        rent_collector: sponsor().pubkey(),
        agent_deposit: AGENT_DEPOSIT,
        agent_voting_time: VOTING_TIME,
        min_voting_quorum_bps: QUORUM_BPS,
        agent_notice_period: NOTICE_PERIOD,
    }
}

pub fn init_ix(authority: &Pubkey) -> Instruction {
    init_ix_with(authority, default_params())
}

pub fn init_ix_with(authority: &Pubkey, params: ConfigParams) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::InitializeConfig { params }.data(),
        property::accounts::InitializeConfig {
            authority: *authority,
            program: pid(),
            program_data: program_data_pda(&pid()),
            config: property_config(),
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
        pid(),
        &property::instruction::UpdateConfig { params }.data(),
        property::accounts::UpdateConfig {
            authority: *authority,
            config: property_config(),
        }
        .to_account_metas(None),
    )
}

pub fn add_agent_ix(
    agent: &Pubkey,
    region_id: u16,
    postcode: &[u8],
    max_deposit: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::AddLettingAgent {
            region_id,
            postcode: postcode.to_vec(),
            max_deposit,
        }
        .data(),
        property::accounts::AddLettingAgent {
            agent: *agent,
            config: property_config(),
            agent_role: role_pda(agent, Role::LettingAgent),
            location: location_pda(region_id, postcode),
            agent_entry: agent_pda(agent),
            xcav_mint: xcav_mint(),
            agent_token: token_acc(agent),
            vault: vault(),
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

pub fn remove_agent_ix(agent: &Pubkey, postcode: &[u8]) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::RemoveLettingAgent {
            postcode: postcode.to_vec(),
        }
        .data(),
        property::accounts::RemoveLettingAgent {
            agent: *agent,
            config: property_config(),
            agent_entry: agent_pda(agent),
            rent_receiver: *agent,
            xcav_mint: xcav_mint(),
            agent_token: token_acc(agent),
            vault: vault(),
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}

// --- election / resignation instruction builders ---

pub fn claim_property_ix(agent: &Pubkey, asset_id: u64, round: u64) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::ClaimProperty { asset_id, round }.data(),
        property::accounts::ClaimProperty {
            agent: *agent,
            payer: *agent,
            config: property_config(),
            agent_role: role_pda(agent, Role::LettingAgent),
            agent_entry: agent_pda(agent),
            property: mkt_property_pda(asset_id),
            letting: letting_pda(asset_id),
            candidacy: candidacy_pda(asset_id, round, agent),
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

pub fn vote_agent_ix(
    voter: &Pubkey,
    asset_id: u64,
    round: u64,
    choice: &Pubkey,
    previous: Option<&Pubkey>,
    amount: u32,
) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::VoteOnAgent { asset_id, amount }.data(),
        property::accounts::VoteOnAgent {
            voter: *voter,
            payer: sponsor().pubkey(),
            voter_role: role_pda(voter, Role::RealEstateInvestor),
            letting: letting_pda(asset_id),
            holding: holding_pda(asset_id, voter),
            vote_record: agent_vote_pda(asset_id, round, voter),
            candidacy: candidacy_pda(asset_id, round, choice),
            previous_candidacy: previous.map(|agent| candidacy_pda(asset_id, round, agent)),
            cpi_auth: cpi_auth(),
            marketplace_program: mid(),
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

pub fn finalize_election_ix(
    cranker: &Pubkey,
    asset_id: u64,
    round: u64,
    winner: Option<&Pubkey>,
    candidates: &[Pubkey],
) -> Instruction {
    let mut accounts = property::accounts::FinalizeAgentElection {
        cranker: *cranker,
        letting: letting_pda(asset_id),
        property: mkt_property_pda(asset_id),
        winner_entry: winner.map(agent_pda),
    }
    .to_account_metas(None);
    for agent in candidates {
        accounts.push(AccountMeta::new(candidacy_pda(asset_id, round, agent), false));
    }
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::FinalizeAgentElection { asset_id }.data(),
        accounts,
    )
}

pub fn close_candidacy_ix(
    cranker: &Pubkey,
    rent_payer: &Pubkey,
    asset_id: u64,
    round: u64,
    agent: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::CloseAgentCandidacy {
            asset_id,
            round,
            agent: *agent,
        }
        .data(),
        property::accounts::CloseAgentCandidacy {
            cranker: *cranker,
            rent_payer: *rent_payer,
            letting: letting_pda(asset_id),
            candidacy: candidacy_pda(asset_id, round, agent),
        }
        .to_account_metas(None),
    )
}

pub fn unlock_votes_ix(voter: &Pubkey, asset_id: u64, round: u64) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::UnlockAgentVotes { asset_id, round }.data(),
        property::accounts::UnlockAgentVotes {
            voter: *voter,
            rent_payer: sponsor().pubkey(),
            letting: letting_pda(asset_id),
            holding: holding_pda(asset_id, voter),
            vote_record: agent_vote_pda(asset_id, round, voter),
            cpi_auth: cpi_auth(),
            marketplace_program: mid(),
        }
        .to_account_metas(None),
    )
}

pub fn resign_ix(agent: &Pubkey, asset_id: u64) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::Resign { asset_id }.data(),
        property::accounts::Resign {
            agent: *agent,
            config: property_config(),
            letting: letting_pda(asset_id),
            notice: resignation_pda(asset_id),
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

pub fn finalize_resignation_ix(cranker: &Pubkey, resigner: &Pubkey, asset_id: u64) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::FinalizeResignation { asset_id }.data(),
        property::accounts::FinalizeResignation {
            cranker: *cranker,
            rent_payer: *resigner,
            letting: letting_pda(asset_id),
            property: mkt_property_pda(asset_id),
            agent_entry: agent_pda(resigner),
            notice: resignation_pda(asset_id),
        }
        .to_account_metas(None),
    )
}

// --- state readers ---

pub fn agent_of(svm: &LiteSVM, wallet: &Pubkey) -> LettingAgent {
    let acc = svm.get_account(&agent_pda(wallet)).unwrap();
    LettingAgent::try_deserialize(&mut acc.data.as_slice()).unwrap()
}

pub fn config_of(svm: &LiteSVM) -> PropertyConfig {
    let acc = svm.get_account(&property_config()).unwrap();
    PropertyConfig::try_deserialize(&mut acc.data.as_slice()).unwrap()
}

pub fn letting_of(svm: &LiteSVM, asset_id: u64) -> PropertyLetting {
    let acc = svm.get_account(&letting_pda(asset_id)).unwrap();
    PropertyLetting::try_deserialize(&mut acc.data.as_slice()).unwrap()
}

pub fn candidacy_of(svm: &LiteSVM, asset_id: u64, round: u64, agent: &Pubkey) -> AgentCandidacy {
    let acc = svm.get_account(&candidacy_pda(asset_id, round, agent)).unwrap();
    AgentCandidacy::try_deserialize(&mut acc.data.as_slice()).unwrap()
}

pub fn holding_of(svm: &LiteSVM, asset_id: u64, owner: &Pubkey) -> ShareHolding {
    let acc = svm.get_account(&holding_pda(asset_id, owner)).unwrap();
    ShareHolding::try_deserialize(&mut acc.data.as_slice()).unwrap()
}

pub fn notice_of(svm: &LiteSVM, asset_id: u64) -> ResignationNotice {
    let acc = svm.get_account(&resignation_pda(asset_id)).unwrap();
    ResignationNotice::try_deserialize(&mut acc.data.as_slice()).unwrap()
}

pub fn account_gone(svm: &LiteSVM, address: &Pubkey) -> bool {
    match svm.get_account(address) {
        None => true,
        Some(acc) => acc.data.is_empty(),
    }
}

/// Move the clock forward.
pub fn warp(svm: &mut LiteSVM, secs: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += secs;
    svm.set_sysvar(&clock);
}

// --- setup ---

/// Loads the roles and property programs, seeds the XCAV mint, initializes
/// both configs, and returns (svm, admin, authority). The admin can hand out
/// roles.
pub fn setup() -> (LiteSVM, Keypair, Keypair) {
    let mut svm = LiteSVM::new();
    svm.add_program(
        roles_id(),
        include_bytes!("../../../../target/deploy/xcavate_whitelist.so"),
    )
    .unwrap();
    svm.add_program(
        pid(),
        include_bytes!("../../../../target/deploy/property.so"),
    )
    .unwrap();
    svm.add_program(
        mid(),
        include_bytes!("../../../../target/deploy/marketplace.so"),
    )
    .unwrap();
    set_mint(&mut svm);

    let authority = funded(&mut svm);
    svm.airdrop(&sponsor().pubkey(), 100_000_000_000).unwrap();
    bind_upgrade_authority(&mut svm, &roles_id(), &authority.pubkey());
    bind_upgrade_authority(&mut svm, &pid(), &authority.pubkey());
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

/// A SOL-funded, XCAV-holding keypair with the LettingAgent role.
pub fn new_agent(svm: &mut LiteSVM, admin: &Keypair) -> Keypair {
    let kp = actor(svm);
    ok(
        svm,
        roles_assign_ix(&admin.pubkey(), &kp.pubkey(), Role::LettingAgent),
        admin,
        &[admin],
    );
    kp
}

/// A SOL-funded keypair with the RealEstateInvestor role and a seeded share
/// holding on the property.
pub fn new_holder(svm: &mut LiteSVM, admin: &Keypair, asset_id: u64, shares: u32) -> Keypair {
    let kp = funded(svm);
    ok(
        svm,
        roles_assign_ix(&admin.pubkey(), &kp.pubkey(), Role::RealEstateInvestor),
        admin,
        &[admin],
    );
    seed_holding(svm, asset_id, &kp.pubkey(), shares);
    kp
}
