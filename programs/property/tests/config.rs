//! Config lifecycle: upgrade-authority-bound initialization, parameter
//! updates, and the two-step authority handover.

mod common;
use common::*;

use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{InstructionData, ToAccountMetas};

fn update_authority_ix(authority: &Pubkey, new_authority: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::UpdateAuthority { new_authority }.data(),
        property::accounts::UpdateAuthority {
            authority: *authority,
            config: property_config(),
        }
        .to_account_metas(None),
    )
}

fn accept_authority_ix(new_authority: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        pid(),
        &property::instruction::AcceptAuthority {}.data(),
        property::accounts::AcceptAuthority {
            new_authority: *new_authority,
            config: property_config(),
        }
        .to_account_metas(None),
    )
}

#[test]
fn initialize_sets_params_and_vault() {
    let (svm, _admin, authority) = setup();
    let config = config_of(&svm);
    assert_eq!(config.authority, authority.pubkey());
    assert_eq!(config.pending_authority, None);
    assert_eq!(config.xcav_mint, xcav_mint());
    assert_eq!(config.treasury, treasury());
    assert_eq!(config.agent_deposit, AGENT_DEPOSIT);
    assert_eq!(vault_balance(&svm), 0);
}

#[test]
fn update_config_is_authority_only() {
    let (mut svm, _admin, authority) = setup();
    let outsider = funded(&mut svm);

    let mut params = default_params();
    params.agent_deposit = AGENT_DEPOSIT * 2;
    fails_with(
        &mut svm,
        update_config_ix(&outsider.pubkey(), params.clone()),
        &outsider,
        &[&outsider],
        "NotAuthority",
    );
    ok(
        &mut svm,
        update_config_ix(&authority.pubkey(), params),
        &authority,
        &[&authority],
    );
    assert_eq!(config_of(&svm).agent_deposit, AGENT_DEPOSIT * 2);
}

#[test]
fn update_rejects_broken_params() {
    let (mut svm, _admin, authority) = setup();
    let mut params = default_params();
    params.agent_deposit = 0;
    fails_with(
        &mut svm,
        update_config_ix(&authority.pubkey(), params),
        &authority,
        &[&authority],
        "InvalidConfig",
    );
}

#[test]
fn authority_handover_takes_two_steps() {
    let (mut svm, _admin, authority) = setup();
    let successor = funded(&mut svm);

    ok(
        &mut svm,
        update_authority_ix(&authority.pubkey(), successor.pubkey()),
        &authority,
        &[&authority],
    );
    // Still the old authority until the successor accepts.
    assert_eq!(config_of(&svm).authority, authority.pubkey());

    ok(
        &mut svm,
        accept_authority_ix(&successor.pubkey()),
        &successor,
        &[&successor],
    );
    let config = config_of(&svm);
    assert_eq!(config.authority, successor.pubkey());
    assert_eq!(config.pending_authority, None);

    // The old key is out.
    fails_with(
        &mut svm,
        update_config_ix(&authority.pubkey(), default_params()),
        &authority,
        &[&authority],
        "NotAuthority",
    );
}

#[test]
fn accept_requires_the_pending_key() {
    let (mut svm, _admin, authority) = setup();
    let successor = funded(&mut svm);
    let impostor = funded(&mut svm);

    ok(
        &mut svm,
        update_authority_ix(&authority.pubkey(), successor.pubkey()),
        &authority,
        &[&authority],
    );
    fails_with(
        &mut svm,
        accept_authority_ix(&impostor.pubkey()),
        &impostor,
        &[&impostor],
        "NotPendingAuthority",
    );
}
