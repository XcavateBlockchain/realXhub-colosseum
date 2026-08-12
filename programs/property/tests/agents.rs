//! The letting agent registry: per-location registration with its deposit,
//! the one-region rule, and the exit path that refunds and closes.

mod common;
use common::*;

const REGION: u16 = 1;

fn setup_with_location() -> (LiteSVM, Keypair, Keypair) {
    let (mut svm, admin, authority) = setup();
    seed_location(&mut svm, REGION, POSTCODE);
    seed_location(&mut svm, REGION, POSTCODE_B);
    (svm, admin, authority)
}

#[test]
fn add_agent_locks_deposit_and_creates_entry() {
    let (mut svm, admin, _authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);

    let before = xcav_balance(&svm, &agent.pubkey());
    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
    );

    let entry = agent_of(&svm, &agent.pubkey());
    assert_eq!(entry.wallet, agent.pubkey());
    assert_eq!(entry.region_id, REGION);
    assert_eq!(entry.locations.len(), 1);
    assert_eq!(entry.locations[0].postcode, POSTCODE.to_vec());
    assert_eq!(entry.locations[0].assigned_count, 0);
    assert_eq!(entry.locations[0].deposit, AGENT_DEPOSIT);
    assert_eq!(xcav_balance(&svm, &agent.pubkey()), before - AGENT_DEPOSIT);
    assert_eq!(vault_balance(&svm), AGENT_DEPOSIT);
}

#[test]
fn second_location_joins_the_same_entry() {
    let (mut svm, admin, _authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);

    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
    );
    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE_B, u64::MAX),
        &agent,
        &[&agent],
    );

    let entry = agent_of(&svm, &agent.pubkey());
    assert_eq!(entry.locations.len(), 2);
    assert_eq!(vault_balance(&svm), 2 * AGENT_DEPOSIT);
}

#[test]
fn add_requires_the_agent_role() {
    let (mut svm, _admin, _authority) = setup_with_location();
    let outsider = actor(&mut svm);
    fails_with(
        &mut svm,
        add_agent_ix(&outsider.pubkey(), REGION, POSTCODE, u64::MAX),
        &outsider,
        &[&outsider],
        "AccountNotInitialized",
    );
}

#[test]
fn add_requires_a_registered_location() {
    let (mut svm, admin, _authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);
    fails_with(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, b"NOWHERE", u64::MAX),
        &agent,
        &[&agent],
        "AccountNotInitialized",
    );
}

#[test]
fn add_rejects_a_second_region() {
    let (mut svm, admin, _authority) = setup_with_location();
    seed_location(&mut svm, 2, POSTCODE);
    let agent = new_agent(&mut svm, &admin);

    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
    );
    fails_with(
        &mut svm,
        add_agent_ix(&agent.pubkey(), 2, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
        "WrongRegion",
    );
}

#[test]
fn add_rejects_a_duplicate_location() {
    let (mut svm, admin, _authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);

    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
    );
    fails_with(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
        "AlreadyInLocation",
    );
}

#[test]
fn add_stops_at_the_location_cap() {
    let (mut svm, admin, _authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);

    let postcodes: Vec<String> = (0..11).map(|i| format!("L{i}")).collect();
    for code in &postcodes {
        seed_location(&mut svm, REGION, code.as_bytes());
    }
    for code in postcodes.iter().take(10) {
        ok(
            &mut svm,
            add_agent_ix(&agent.pubkey(), REGION, code.as_bytes(), u64::MAX),
            &agent,
            &[&agent],
        );
    }
    fails_with(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, postcodes[10].as_bytes(), u64::MAX),
        &agent,
        &[&agent],
        "TooManyLocations",
    );
}

#[test]
fn add_respects_the_deposit_cap() {
    let (mut svm, admin, _authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);
    fails_with(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, AGENT_DEPOSIT - 1),
        &agent,
        &[&agent],
        "DepositTooHigh",
    );
}

#[test]
fn remove_refunds_and_closes_the_empty_entry() {
    let (mut svm, admin, _authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);

    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
    );
    let before = xcav_balance(&svm, &agent.pubkey());
    ok(
        &mut svm,
        remove_agent_ix(&agent.pubkey(), POSTCODE),
        &agent,
        &[&agent],
    );

    assert_eq!(xcav_balance(&svm, &agent.pubkey()), before + AGENT_DEPOSIT);
    assert_eq!(vault_balance(&svm), 0);
    // Last location gone: the entry account closed and refunded its rent.
    let acc = svm.get_account(&agent_pda(&agent.pubkey()));
    assert!(acc.is_none() || acc.unwrap().data.is_empty());
}

#[test]
fn remove_keeps_the_entry_while_locations_remain() {
    let (mut svm, admin, _authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);

    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
    );
    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE_B, u64::MAX),
        &agent,
        &[&agent],
    );
    ok(
        &mut svm,
        remove_agent_ix(&agent.pubkey(), POSTCODE),
        &agent,
        &[&agent],
    );

    let entry = agent_of(&svm, &agent.pubkey());
    assert_eq!(entry.locations.len(), 1);
    assert_eq!(entry.locations[0].postcode, POSTCODE_B.to_vec());
    assert_eq!(vault_balance(&svm), AGENT_DEPOSIT);
}

#[test]
fn remove_rejects_an_unknown_location() {
    let (mut svm, admin, _authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);

    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
    );
    fails_with(
        &mut svm,
        remove_agent_ix(&agent.pubkey(), POSTCODE_B),
        &agent,
        &[&agent],
        "NotInLocation",
    );
}

#[test]
fn remove_needs_no_role() {
    // The exit stays open after the role is revoked, per the house rule that
    // revocation blocks new activity, never exits.
    let (mut svm, admin, _authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);

    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
    );
    ok(
        &mut svm,
        roles_revoke_ix(&admin.pubkey(), &agent.pubkey(), Role::LettingAgent),
        &admin,
        &[&admin],
    );
    ok(
        &mut svm,
        remove_agent_ix(&agent.pubkey(), POSTCODE),
        &agent,
        &[&agent],
    );
    assert_eq!(vault_balance(&svm), 0);
}

#[test]
fn reregistering_after_a_full_exit_starts_fresh() {
    let (mut svm, admin, _authority) = setup_with_location();
    seed_location(&mut svm, 2, POSTCODE_B);
    let agent = new_agent(&mut svm, &admin);

    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
    );
    ok(
        &mut svm,
        remove_agent_ix(&agent.pubkey(), POSTCODE),
        &agent,
        &[&agent],
    );

    // The closed entry left no trace, so the agent may pick a new region.
    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), 2, POSTCODE_B, u64::MAX),
        &agent,
        &[&agent],
    );
    let entry = agent_of(&svm, &agent.pubkey());
    assert_eq!(entry.region_id, 2);
    assert_eq!(entry.locations.len(), 1);
    assert_eq!(vault_balance(&svm), AGENT_DEPOSIT);
}

#[test]
fn refund_uses_the_deposit_recorded_at_registration() {
    let (mut svm, admin, authority) = setup_with_location();
    let agent = new_agent(&mut svm, &admin);

    ok(
        &mut svm,
        add_agent_ix(&agent.pubkey(), REGION, POSTCODE, u64::MAX),
        &agent,
        &[&agent],
    );
    // The config reprices later; the refund must not.
    let mut params = default_params();
    params.agent_deposit = AGENT_DEPOSIT * 3;
    ok(
        &mut svm,
        update_config_ix(&authority.pubkey(), params),
        &authority,
        &[&authority],
    );

    let before = xcav_balance(&svm, &agent.pubkey());
    ok(
        &mut svm,
        remove_agent_ix(&agent.pubkey(), POSTCODE),
        &agent,
        &[&agent],
    );
    assert_eq!(xcav_balance(&svm, &agent.pubkey()), before + AGENT_DEPOSIT);
}
