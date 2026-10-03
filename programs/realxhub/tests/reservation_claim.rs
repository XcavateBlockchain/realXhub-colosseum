mod common;
use anchor_lang::solana_program::{clock::Clock, program_pack::Pack};
use anchor_lang::AccountSerialize;
use anchor_spl::token::spl_token::state::{Account as SplAccount, AccountState};
use common::*;
use realxhub::state::{
    HubFunding, HubReservation, PaymentReservation, ReservationClaim, ReservationSale,
};

fn funding(id: u64) -> Pubkey {
    address(&[realxhub::FUNDING_SEED, &id.to_le_bytes()])
}
fn funding_of(svm: &LiteSVM, id: u64) -> HubFunding {
    HubFunding::try_deserialize(&mut &svm.get_account(&funding(id)).unwrap().data[..]).unwrap()
}
fn sale(id: u64) -> Pubkey {
    address(&[realxhub::RESERVATION_SALE_SEED, &id.to_le_bytes()])
}
fn reservation(id: u64, buyer: &Pubkey) -> Pubkey {
    address(&[
        realxhub::HUB_RESERVATION_SEED,
        &id.to_le_bytes(),
        buyer.as_ref(),
    ])
}
fn ledger(buyer: &Pubkey) -> Pubkey {
    address(&[
        realxhub::PAYMENT_RESERVATION_SEED,
        ata(buyer, &payment_mint()).as_ref(),
    ])
}
fn receipt(id: u64, buyer: &Pubkey) -> Pubkey {
    address(&[
        realxhub::RESERVATION_CLAIM_SEED,
        &id.to_le_bytes(),
        buyer.as_ref(),
    ])
}
fn sale_of(svm: &LiteSVM, id: u64) -> ReservationSale {
    ReservationSale::try_deserialize(&mut &svm.get_account(&sale(id)).unwrap().data[..]).unwrap()
}
fn reservation_of(svm: &LiteSVM, id: u64, buyer: &Pubkey) -> HubReservation {
    HubReservation::try_deserialize(
        &mut &svm.get_account(&reservation(id, buyer)).unwrap().data[..],
    )
    .unwrap()
}
fn receipt_of(svm: &LiteSVM, id: u64, buyer: &Pubkey) -> ReservationClaim {
    ReservationClaim::try_deserialize(&mut &svm.get_account(&receipt(id, buyer)).unwrap().data[..])
        .unwrap()
}
fn promised(svm: &LiteSVM, buyer: &Pubkey) -> u64 {
    PaymentReservation::try_deserialize(&mut &svm.get_account(&ledger(buyer)).unwrap().data[..])
        .unwrap()
        .amount
}
fn time(svm: &mut LiteSVM, now: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = now;
    svm.set_sysvar(&clock);
}
fn open_ix(operator: &Pubkey, id: u64) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::OpenReservations { hub_id: id }.data(),
        realxhub::accounts::OpenReservations {
            operator: *operator,
            hub: hub(id),
            operator_role: region_common::role_pda(operator, Role::RegionalOperator),
            region: region_common::region_pda(1),
            sale: sale(id),
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}
fn reserve_ix(buyer: &Pubkey, id: u64, amount: u64) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::ReserveTokens {
            hub_id: id,
            amount,
            max_total_cost: PRICE * amount,
        }
        .data(),
        realxhub::accounts::ReserveTokens {
            buyer: *buyer,
            config: config(),
            hub: hub(id),
            buyer_role: region_common::role_pda(buyer, Role::RealEstateInvestor),
            sale: sale(id),
            payment_mint: payment_mint(),
            buyer_payment: ata(buyer, &payment_mint()),
            reservation: reservation(id, buyer),
            payment_reservation: ledger(buyer),
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}
fn claim_ix(operator: &Pubkey, buyer: &Pubkey, id: u64, cap: u64) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::ClaimReservedTokens {
            hub_id: id,
            max_total_cost: cap,
        }
        .data(),
        realxhub::accounts::ClaimReservedTokens {
            buyer: *buyer,
            config: config(),
            hub: hub(id),
            buyer_role: region_common::role_pda(buyer, Role::RealEstateInvestor),
            sale: sale(id),
            reservation: reservation(id, buyer),
            payment_reservation: ledger(buyer),
            payment_mint: payment_mint(),
            buyer_payment: ata(buyer, &payment_mint()),
            payment_vault: payment_vault(id),
            hub_mint: hub_mint(id),
            token_vault: token_vault(id),
            buyer_token: ata(buyer, &hub_mint(id)),
            records: realxhub::accounts::ClaimRecords {
                buyer: *buyer,
                hub: hub(id),
                claim: receipt(id, buyer),
                funding: funding(id),
                system_program: SYS,
            },
            operator: *operator,
            operator_payment: ata(operator, &payment_mint()),
            token_program: TOKEN,
            associated_token_program: ATA,
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}
fn release_ix(caller: &Pubkey, buyer: &Pubkey, id: u64) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::ReleaseReservation {
            hub_id: id,
            buyer: *buyer,
        }
        .data(),
        realxhub::accounts::ReleaseReservation {
            cranker: *caller,
            hub: hub(id),
            sale: sale(id),
            reservation: reservation(id, buyer),
            payment_reservation: ledger(buyer),
        }
        .to_account_metas(None),
    )
}
fn reopen_ix(caller: &Pubkey, id: u64) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::ReopenReservations { hub_id: id }.data(),
        realxhub::accounts::ReopenReservations {
            cranker: *caller,
            hub: hub(id),
            sale: sale(id),
        }
        .to_account_metas(None),
    )
}
fn finalize_reservations_ix(caller: &Pubkey, id: u64) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::FinalizeReservations { hub_id: id }.data(),
        realxhub::accounts::FinalizeReservations {
            cranker: *caller,
            hub: hub(id),
            sale: sale(id),
        }
        .to_account_metas(None),
    )
}
fn investor(f: &mut Fixture) -> Keypair {
    let buyer = region_common::funded(&mut f.svm);
    region_common::ok(
        &mut f.svm,
        region_common::roles_assign_ix(
            &f.admin.pubkey(),
            &buyer.pubkey(),
            Role::RealEstateInvestor,
        ),
        &f.admin,
        &[&f.admin],
    );
    set_payment(&mut f.svm, &buyer.pubkey(), PAYMENT_BALANCE);
    buyer
}
fn full_round(f: &mut Fixture) -> Keypair {
    let other = investor(f);
    reach_listed(f, 0);
    ok(&mut f.svm, open_ix(&f.operator.pubkey(), 0), &f.operator);
    ok(&mut f.svm, reserve_ix(&f.buyer.pubkey(), 0, 4), &f.buyer);
    ok(&mut f.svm, reserve_ix(&other.pubkey(), 0, 6), &other);
    other
}
fn snapshot(svm: &LiteSVM, id: u64, buyer: &Pubkey) -> Vec<Option<region_common::Account>> {
    [
        hub(id),
        sale(id),
        reservation(id, buyer),
        ledger(buyer),
        receipt(id, buyer),
        payment_vault(id),
        token_vault(id),
        ata(buyer, &payment_mint()),
        ata(buyer, &hub_mint(id)),
        funding(id),
        ata(&hub_of(svm, id).operator, &payment_mint()),
    ]
    .iter()
    .map(|key| svm.get_account(key))
    .collect()
}
fn expire(svm: &mut LiteSVM) {
    let deadline = sale_of(svm, 0).claim_deadline;
    time(svm, deadline);
}
fn pay_first(f: &mut Fixture) {
    ok(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 4),
        &f.buyer,
    );
}
fn release_and_reopen(f: &mut Fixture, unpaid: &Keypair) {
    expire(&mut f.svm);
    ok(
        &mut f.svm,
        release_ix(&f.verifier.pubkey(), &unpaid.pubkey(), 0),
        &f.verifier,
    );
    ok(&mut f.svm, reopen_ix(&f.verifier.pubkey(), 0), &f.verifier);
}

#[test]
fn claims_collect_exact_payment_deliver_tokens_and_fund_only_after_every_claim() {
    let mut f = setup();
    let other = full_round(&mut f);
    let deadline = sale_of(&f.svm, 0).claim_deadline;
    pay_first(&mut f);
    assert!(f
        .svm
        .get_account(&ata(&f.operator.pubkey(), &payment_mint()))
        .is_none());
    assert!(!funding_of(&f.svm, 0).first_tranche_released);
    let h = hub_of(&f.svm, 0);
    assert_eq!(
        (h.status, h.tokens_sold, h.tokens_claimed, h.total_paid),
        (HubStatus::Claiming, 4, 4, PRICE * 4)
    );
    assert_eq!(
        balance(&f.svm, ata(&f.buyer.pubkey(), &payment_mint())),
        PAYMENT_BALANCE - PRICE * 4
    );
    assert_eq!(balance(&f.svm, ata(&f.buyer.pubkey(), &hub_mint(0))), 4);
    assert_eq!(balance(&f.svm, token_vault(0)), 6);
    assert_eq!(promised(&f.svm, &f.buyer.pubkey()), 0);
    assert_eq!(sale_of(&f.svm, 0).reserved_tokens, 6);
    assert_eq!(reservation_of(&f.svm, 0, &f.buyer.pubkey()).amount, 0);
    let claim = receipt_of(&f.svm, 0, &f.buyer.pubkey());
    assert_eq!(
        (claim.hub, claim.buyer, claim.amount, claim.paid),
        (hub(0), f.buyer.pubkey(), 4, PRICE * 4)
    );
    assert!(f.svm.get_account(&position(0, &f.buyer.pubkey())).is_none());
    assert_eq!(sale_of(&f.svm, 0).claim_deadline, deadline);
    ok(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &other.pubkey(), 0, PRICE * 6),
        &other,
    );
    let h = hub_of(&f.svm, 0);
    assert_eq!(
        (h.status, h.tokens_sold, h.tokens_claimed, h.total_paid),
        (HubStatus::Funded, SUPPLY, SUPPLY, PRICE * SUPPLY)
    );
    assert_eq!(sale_of(&f.svm, 0).reserved_tokens, 0);
    assert_eq!(promised(&f.svm, &other.pubkey()), 0);
    assert_eq!(balance(&f.svm, payment_vault(0)), PRICE * SUPPLY / 2);
    assert_eq!(
        balance(&f.svm, ata(&f.operator.pubkey(), &payment_mint())),
        PRICE * SUPPLY / 2
    );
    let settlement = funding_of(&f.svm, 0);
    assert_eq!(settlement.hub, hub(0));
    assert!(settlement.first_tranche_released);
    assert_eq!(settlement.first_tranche_amount, PRICE * SUPPLY / 2);
    assert_eq!(
        settlement.first_tranche_released_at,
        f.svm.get_sysvar::<Clock>().unix_timestamp
    );
    assert_eq!(balance(&f.svm, bond_vault(0)), BOND);
    assert_eq!(balance(&f.svm, token_vault(0)), 0);
    assert_eq!(balance(&f.svm, ata(&other.pubkey(), &hub_mint(0))), 6);
    assert!(process(&mut f.svm, reopen_ix(&f.verifier.pubkey(), 0), &f.verifier).is_err());
}

#[test]
fn a_duplicate_claim_cannot_charge_or_deliver_again() {
    let mut f = setup();
    full_round(&mut f);
    pay_first(&mut f);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    fails_with(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 4),
        &f.buyer,
        "EmptyPosition",
    );
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
}

#[test]
fn claiming_before_the_last_reservation_leaves_all_accounts_unchanged() {
    let mut f = setup();
    reach_listed(&mut f, 0);
    ok(&mut f.svm, open_ix(&f.operator.pubkey(), 0), &f.operator);
    ok(&mut f.svm, reserve_ix(&f.buyer.pubkey(), 0, 4), &f.buyer);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    fails_with(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 4),
        &f.buyer,
        "InvalidStatus",
    );
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
}

#[test]
fn cost_cap_and_live_insufficient_balance_roll_back_claim_initialization() {
    let mut f = setup();
    full_round(&mut f);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    fails_with(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 4 - 1),
        &f.buyer,
        "CostTooHigh",
    );
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    set_payment(&mut f.svm, &f.buyer.pubkey(), PRICE * 4 - 1);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    assert!(process(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 4),
        &f.buyer
    )
    .is_err());
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
}

#[test]
fn failed_token_delivery_rolls_back_the_preceding_payment() {
    let mut f = setup();
    full_round(&mut f);
    // Fault injection proves rollback when the second transfer fails after payment.
    let mut vault = f.svm.get_account(&token_vault(0)).unwrap();
    let mut token = SplAccount::unpack(&vault.data).unwrap();
    token.amount = 0;
    token.pack_into_slice(&mut vault.data);
    f.svm.set_account(token_vault(0), vault).unwrap();
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    assert!(process(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 4),
        &f.buyer
    )
    .is_err());
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
}

#[test]
fn claim_deadline_is_exclusive_and_the_previous_second_is_allowed() {
    for before_deadline in [false, true] {
        let mut f = setup();
        full_round(&mut f);
        let deadline = sale_of(&f.svm, 0).claim_deadline;
        time(&mut f.svm, deadline - i64::from(before_deadline));
        if before_deadline {
            pay_first(&mut f);
        } else {
            let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
            fails_with(
                &mut f.svm,
                claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 4),
                &f.buyer,
                "SaleExpired",
            );
            assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
        }
    }
}

#[test]
fn collecting_new_payment_requires_current_investor_compliance() {
    let mut f = setup();
    full_round(&mut f);
    region_common::set_permission(
        &mut f.svm,
        &f.authority,
        &f.buyer.pubkey(),
        Role::RealEstateInvestor,
        false,
    );
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    fails_with(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 4),
        &f.buyer,
        "NotCompliant",
    );
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
}

#[test]
fn claims_reject_other_hubs_positions_ledgers_vaults_and_recipients() {
    let mut f = setup();
    let other = full_round(&mut f);
    reach_listed(&mut f, 1);
    ok(&mut f.svm, open_ix(&f.operator.pubkey(), 1), &f.operator);
    ok(
        &mut f.svm,
        reserve_ix(&f.buyer.pubkey(), 1, SUPPLY),
        &f.buyer,
    );
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    for (index, wrong) in [
        (4, sale(1)),
        (5, reservation(1, &f.buyer.pubkey())),
        (5, reservation(0, &other.pubkey())),
        (6, ledger(&other.pubkey())),
        (9, payment_vault(1)),
        (10, hub_mint(1)),
        (11, token_vault(1)),
        (12, ata(&other.pubkey(), &hub_mint(0))),
        (15, receipt(1, &f.buyer.pubkey())),
    ] {
        let mut ix = claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 4);
        ix.accounts[index].pubkey = wrong;
        assert!(process(&mut f.svm, ix, &f.buyer).is_err());
        assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    }
}

#[test]
fn a_claim_must_collect_from_the_original_payment_account() {
    let mut f = setup();
    full_round(&mut f);
    let another = Pubkey::new_unique();
    let account = f
        .svm
        .get_account(&ata(&f.buyer.pubkey(), &payment_mint()))
        .unwrap();
    f.svm.set_account(another, account).unwrap();
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    let mut ix = claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 4);
    ix.accounts[8].pubkey = another;
    fails_with(&mut f.svm, ix, &f.buyer, "ReservationAccountMismatch");
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
}

#[test]
fn partial_round_reopens_only_after_expiry_and_all_unpaid_promises_are_cleared() {
    let mut f = setup();
    let other = full_round(&mut f);
    pay_first(&mut f);
    fails_with(
        &mut f.svm,
        reopen_ix(&f.verifier.pubkey(), 0),
        &f.verifier,
        "ReservationStillActive",
    );
    expire(&mut f.svm);
    fails_with(
        &mut f.svm,
        reopen_ix(&f.verifier.pubkey(), 0),
        &f.verifier,
        "UnpaidReservationsOutstanding",
    );
    fails_with(
        &mut f.svm,
        finalize_reservations_ix(&f.verifier.pubkey(), 0),
        &f.verifier,
        "PurchasesOutstanding",
    );
    ok(
        &mut f.svm,
        release_ix(&f.verifier.pubkey(), &other.pubkey(), 0),
        &f.verifier,
    );
    let now = f.svm.get_sysvar::<Clock>().unix_timestamp;
    ok(&mut f.svm, reopen_ix(&f.verifier.pubkey(), 0), &f.verifier);
    let h = hub_of(&f.svm, 0);
    assert_eq!(
        (h.status, h.tokens_sold, h.tokens_claimed, h.total_paid),
        (HubStatus::Reserving, 4, 4, PRICE * 4)
    );
    assert_eq!(h.sale_deadline, now + DURATION);
    assert_eq!(
        (
            sale_of(&f.svm, 0).claim_started_at,
            sale_of(&f.svm, 0).claim_deadline
        ),
        (0, 0)
    );
    assert_eq!(balance(&f.svm, payment_vault(0)), PRICE * 4);
    assert_eq!(balance(&f.svm, ata(&f.buyer.pubkey(), &hub_mint(0))), 4);
    assert_eq!(promised(&f.svm, &other.pubkey()), 0);
    let replacement = investor(&mut f);
    fails_with(
        &mut f.svm,
        reserve_ix(&replacement.pubkey(), 0, 7),
        &replacement,
        "InvalidAmount",
    );
    ok(
        &mut f.svm,
        reserve_ix(&replacement.pubkey(), 0, 6),
        &replacement,
    );
    assert_eq!(hub_of(&f.svm, 0).status, HubStatus::Claiming);
    assert_eq!(
        sale_of(&f.svm, 0).claim_deadline,
        now + realxhub::CLAIM_WINDOW_SECONDS
    );
    // The old unpaid record cannot be revived by a new deadline.
    fails_with(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &other.pubkey(), 0, PRICE * 6),
        &other,
        "EmptyPosition",
    );
    ok(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &replacement.pubkey(), 0, PRICE * 6),
        &replacement,
    );
    assert_eq!(hub_of(&f.svm, 0).status, HubStatus::Funded);
    assert_eq!(hub_of(&f.svm, 0).total_paid, PRICE * SUPPLY);
    assert_eq!(balance(&f.svm, token_vault(0)), 0);
}

#[test]
fn an_existing_paid_buyer_can_claim_more_in_a_reopened_round() {
    let mut f = setup();
    let other = full_round(&mut f);
    pay_first(&mut f);
    let first_time = receipt_of(&f.svm, 0, &f.buyer.pubkey()).last_claimed_at;
    release_and_reopen(&mut f, &other);
    ok(&mut f.svm, reserve_ix(&f.buyer.pubkey(), 0, 6), &f.buyer);
    ok(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, PRICE * 6),
        &f.buyer,
    );
    let claim = receipt_of(&f.svm, 0, &f.buyer.pubkey());
    assert_eq!((claim.amount, claim.paid), (SUPPLY, PRICE * SUPPLY));
    assert!(claim.last_claimed_at > first_time);
    assert_eq!(
        balance(&f.svm, ata(&f.buyer.pubkey(), &hub_mint(0))),
        SUPPLY
    );
    assert_eq!(promised(&f.svm, &f.buyer.pubkey()), 0);
}

#[test]
fn a_reopened_campaign_can_renew_after_its_new_reservation_deadline() {
    let mut f = setup();
    let other = full_round(&mut f);
    pay_first(&mut f);
    release_and_reopen(&mut f, &other);
    let deadline = hub_of(&f.svm, 0).sale_deadline;
    fails_with(
        &mut f.svm,
        reopen_ix(&f.verifier.pubkey(), 0),
        &f.verifier,
        "ReservationStillActive",
    );
    time(&mut f.svm, deadline);
    fails_with(
        &mut f.svm,
        reserve_ix(&other.pubkey(), 0, 6),
        &other,
        "SaleExpired",
    );
    ok(&mut f.svm, reopen_ix(&f.verifier.pubkey(), 0), &f.verifier);
    assert_eq!(hub_of(&f.svm, 0).sale_deadline, deadline + DURATION);
    ok(&mut f.svm, reserve_ix(&other.pubkey(), 0, 6), &other);
    ok(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &other.pubkey(), 0, PRICE * 6),
        &other,
    );
    assert_eq!(hub_of(&f.svm, 0).status, HubStatus::Funded);
}

#[test]
fn cleanup_cannot_remove_paid_allocations_or_return_their_operator_bond() {
    let mut f = setup();
    full_round(&mut f);
    pay_first(&mut f);
    expire(&mut f.svm);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    fails_with(
        &mut f.svm,
        release_ix(&f.verifier.pubkey(), &f.buyer.pubkey(), 0),
        &f.verifier,
        "EmptyPosition",
    );
    fails_with(
        &mut f.svm,
        bond_refund_ix(&f.operator.pubkey(), 0),
        &f.operator,
        "InvalidStatus",
    );
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
}

#[test]
fn a_round_with_no_payments_keeps_its_existing_failure_and_bond_refund_path() {
    let mut f = setup();
    full_round(&mut f);
    expire(&mut f.svm);
    fails_with(
        &mut f.svm,
        reopen_ix(&f.verifier.pubkey(), 0),
        &f.verifier,
        "InvalidStatus",
    );
    ok(
        &mut f.svm,
        finalize_reservations_ix(&f.verifier.pubkey(), 0),
        &f.verifier,
    );
    ok(
        &mut f.svm,
        bond_refund_ix(&f.operator.pubkey(), 0),
        &f.operator,
    );
    assert_eq!(hub_of(&f.svm, 0).status, HubStatus::Failed);
    assert_eq!(balance(&f.svm, bond_vault(0)), 0);
}

#[test]
fn claim_account_initialization_survives_an_unsolicited_lamport_deposit() {
    let mut f = setup();
    full_round(&mut f);
    f.svm.airdrop(&receipt(0, &f.buyer.pubkey()), 1).unwrap();
    f.svm.airdrop(&funding(0), 1).unwrap();
    pay_first(&mut f);
    assert_eq!(receipt_of(&f.svm, 0, &f.buyer.pubkey()).amount, 4);
}

#[test]
fn paying_one_hub_releases_only_that_hubs_aggregate_payment_quote() {
    let mut f = setup();
    full_round(&mut f);
    reach_listed(&mut f, 1);
    ok(&mut f.svm, open_ix(&f.operator.pubkey(), 1), &f.operator);
    ok(&mut f.svm, reserve_ix(&f.buyer.pubkey(), 1, 3), &f.buyer);
    assert_eq!(promised(&f.svm, &f.buyer.pubkey()), PRICE * 7);
    pay_first(&mut f);
    assert_eq!(promised(&f.svm, &f.buyer.pubkey()), PRICE * 3);
    assert_eq!(reservation_of(&f.svm, 1, &f.buyer.pubkey()).amount, 3);
    assert_eq!(sale_of(&f.svm, 1).reserved_tokens, 3);
    assert_eq!(hub_of(&f.svm, 1).total_paid, 0);
    assert_eq!(balance(&f.svm, payment_vault(1)), 0);
}

#[test]
fn clearing_only_one_of_several_unpaid_buyers_cannot_reopen_a_round() {
    let mut f = setup();
    let other = investor(&mut f);
    let third = investor(&mut f);
    reach_listed(&mut f, 0);
    ok(&mut f.svm, open_ix(&f.operator.pubkey(), 0), &f.operator);
    ok(&mut f.svm, reserve_ix(&f.buyer.pubkey(), 0, 4), &f.buyer);
    ok(&mut f.svm, reserve_ix(&other.pubkey(), 0, 3), &other);
    ok(&mut f.svm, reserve_ix(&third.pubkey(), 0, 3), &third);
    pay_first(&mut f);
    expire(&mut f.svm);
    ok(
        &mut f.svm,
        release_ix(&f.verifier.pubkey(), &other.pubkey(), 0),
        &f.verifier,
    );
    fails_with(
        &mut f.svm,
        reopen_ix(&f.verifier.pubkey(), 0),
        &f.verifier,
        "UnpaidReservationsOutstanding",
    );
    assert_eq!(sale_of(&f.svm, 0).reserved_tokens, 3);
    ok(
        &mut f.svm,
        release_ix(&f.verifier.pubkey(), &third.pubkey(), 0),
        &f.verifier,
    );
    ok(&mut f.svm, reopen_ix(&f.verifier.pubkey(), 0), &f.verifier);
    assert_eq!(hub_of(&f.svm, 0).status, HubStatus::Reserving);
}

fn tranche_ix(caller: &Pubkey, operator: &Pubkey, id: u64) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::ReleaseFirstTranche { hub_id: id }.data(),
        realxhub::accounts::ReleaseFirstTranche {
            cranker: *caller,
            config: config(),
            hub: hub(id),
            sale: sale(id),
            payment_mint: payment_mint(),
            payment_vault: payment_vault(id),
            operator: *operator,
            operator_payment: ata(operator, &payment_mint()),
            funding: funding(id),
            token_program: TOKEN,
            associated_token_program: ATA,
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}
fn store_state(svm: &mut LiteSVM, key: Pubkey, state: &impl AccountSerialize) {
    let mut account = svm.get_account(&key).unwrap();
    state.try_serialize(&mut &mut account.data[..]).unwrap();
    svm.set_account(key, account).unwrap();
}
fn set_token_balance(svm: &mut LiteSVM, key: Pubkey, amount: u64) {
    let mut account = svm.get_account(&key).unwrap();
    let mut token = SplAccount::unpack(&account.data).unwrap();
    token.amount = amount;
    token.pack_into_slice(&mut account.data);
    svm.set_account(key, account).unwrap();
}
fn funded_before_tranches(f: &mut Fixture) {
    let other = full_round(f);
    // Simulate persisted accounts from step 1, where funding did not release
    // a tranche. Current successful final claims already release automatically.
    let mut h = hub_of(&f.svm, 0);
    h.status = HubStatus::Funded;
    h.tokens_sold = SUPPLY;
    h.tokens_claimed = SUPPLY;
    h.total_paid = PRICE * SUPPLY;
    store_state(&mut f.svm, hub(0), &h);
    let mut s = sale_of(&f.svm, 0);
    s.reserved_tokens = 0;
    store_state(&mut f.svm, sale(0), &s);
    for buyer in [f.buyer.pubkey(), other.pubkey()] {
        let mut r = reservation_of(&f.svm, 0, &buyer);
        r.amount = 0;
        r.quoted_payment = 0;
        store_state(&mut f.svm, reservation(0, &buyer), &r);
        let mut payment = PaymentReservation::try_deserialize(
            &mut &f.svm.get_account(&ledger(&buyer)).unwrap().data[..],
        )
        .unwrap();
        payment.amount = 0;
        store_state(&mut f.svm, ledger(&buyer), &payment);
    }
    set_token_balance(&mut f.svm, payment_vault(0), PRICE * SUPPLY);
    set_token_balance(&mut f.svm, token_vault(0), 0);
}

#[test]
fn the_first_tranche_cannot_be_released_again_by_any_caller() {
    let mut f = setup();
    let other = full_round(&mut f);
    pay_first(&mut f);
    ok(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &other.pubkey(), 0, PRICE * 6),
        &other,
    );
    let before = snapshot(&f.svm, 0, &other.pubkey());
    for caller in [&f.verifier, &f.operator, &f.buyer] {
        fails_with(
            &mut f.svm,
            tranche_ix(&caller.pubkey(), &f.operator.pubkey(), 0),
            caller,
            "FirstTrancheAlreadyReleased",
        );
        assert_eq!(snapshot(&f.svm, 0, &other.pubkey()), before);
    }
    assert!(process(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &other.pubkey(), 0, PRICE * 6),
        &other
    )
    .is_err());
    assert_eq!(snapshot(&f.svm, 0, &other.pubkey()), before);
}

#[test]
fn a_partially_claimed_campaign_cannot_release_the_first_tranche() {
    let mut f = setup();
    let other = full_round(&mut f);
    pay_first(&mut f);
    assert!(process(
        &mut f.svm,
        tranche_ix(&f.verifier.pubkey(), &f.operator.pubkey(), 0),
        &f.verifier
    )
    .is_err());
    expire(&mut f.svm);
    ok(
        &mut f.svm,
        release_ix(&f.verifier.pubkey(), &other.pubkey(), 0),
        &f.verifier,
    );
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    fails_with(
        &mut f.svm,
        tranche_ix(&f.verifier.pubkey(), &f.operator.pubkey(), 0),
        &f.verifier,
        "InvalidStatus",
    );
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    assert!(f
        .svm
        .get_account(&ata(&f.operator.pubkey(), &payment_mint()))
        .is_none());
    assert!(!funding_of(&f.svm, 0).first_tranche_released);
}

#[test]
fn a_final_claim_cannot_redirect_the_operator_payout_or_funding_record() {
    let mut f = setup();
    let other = full_round(&mut f);
    pay_first(&mut f);
    reach_listed(&mut f, 1);
    let before = snapshot(&f.svm, 0, &other.pubkey());
    for (index, wrong) in [
        (18, f.buyer.pubkey()),
        (19, ata(&f.buyer.pubkey(), &payment_mint())),
        (16, funding(1)),
        (14, hub(1)),
    ] {
        let mut ix = claim_ix(&f.operator.pubkey(), &other.pubkey(), 0, PRICE * 6);
        ix.accounts[index].pubkey = wrong;
        assert!(process(&mut f.svm, ix, &other).is_err());
        assert_eq!(snapshot(&f.svm, 0, &other.pubkey()), before);
    }
    let first_buyer_before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    let mut ix = claim_ix(&f.operator.pubkey(), &other.pubkey(), 0, PRICE * 6);
    ix.accounts[13].pubkey = f.buyer.pubkey();
    ix.accounts[15].pubkey = receipt(0, &f.buyer.pubkey());
    // Both buyers sign: the program must still bind the grouped receipt to
    // the actual payer, rather than relying on a missing transaction signature.
    region_common::fails_with(
        &mut f.svm,
        ix,
        &other,
        &[&other, &f.buyer],
        "InvalidPosition",
    );
    assert_eq!(snapshot(&f.svm, 0, &other.pubkey()), before);
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), first_buyer_before);
}

#[test]
fn unsolicited_payment_donations_are_retained_and_do_not_enlarge_the_tranche() {
    let mut f = setup();
    let other = full_round(&mut f);
    set_payment(&mut f.svm, &f.operator.pubkey(), 100);
    pay_first(&mut f);
    let donation = anchor_spl::token::spl_token::instruction::transfer_checked(
        &TOKEN,
        &ata(&f.buyer.pubkey(), &payment_mint()),
        &payment_mint(),
        &payment_vault(0),
        &f.buyer.pubkey(),
        &[],
        17,
        6,
    )
    .unwrap();
    ok(&mut f.svm, donation, &f.buyer);
    ok(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &other.pubkey(), 0, PRICE * 6),
        &other,
    );
    assert_eq!(hub_of(&f.svm, 0).total_paid, PRICE * SUPPLY);
    assert_eq!(balance(&f.svm, payment_vault(0)), PRICE * SUPPLY / 2 + 17);
    assert_eq!(
        balance(&f.svm, ata(&f.operator.pubkey(), &payment_mint())),
        100 + PRICE * SUPPLY / 2
    );
    assert_eq!(
        funding_of(&f.svm, 0).first_tranche_amount,
        PRICE * SUPPLY / 2
    );
}

#[test]
fn failed_tranche_delivery_rolls_back_the_final_payment_tokens_and_funding_status() {
    let mut f = setup();
    let other = full_round(&mut f);
    pay_first(&mut f);
    set_payment(&mut f.svm, &f.operator.pubkey(), 0);
    let operator_ata = ata(&f.operator.pubkey(), &payment_mint());
    // Inject a failing destination to exercise the third CPI, after the final
    // buyer's payment and token delivery have both executed.
    let mut account = f.svm.get_account(&operator_ata).unwrap();
    let mut token = SplAccount::unpack(&account.data).unwrap();
    token.state = AccountState::Frozen;
    token.pack_into_slice(&mut account.data);
    f.svm.set_account(operator_ata, account).unwrap();
    let before = snapshot(&f.svm, 0, &other.pubkey());
    assert!(process(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &other.pubkey(), 0, PRICE * 6),
        &other
    )
    .is_err());
    assert_eq!(snapshot(&f.svm, 0, &other.pubkey()), before);
    assert_eq!(hub_of(&f.svm, 0).status, HubStatus::Claiming);
    assert!(!funding_of(&f.svm, 0).first_tranche_released);
    let mut account = f.svm.get_account(&operator_ata).unwrap();
    token.state = AccountState::Initialized;
    token.pack_into_slice(&mut account.data);
    f.svm.set_account(operator_ata, account).unwrap();
    ok(
        &mut f.svm,
        claim_ix(&f.operator.pubkey(), &other.pubkey(), 0, PRICE * 6),
        &other,
    );
    assert!(funding_of(&f.svm, 0).first_tranche_released);
}

#[test]
fn odd_payment_units_stay_in_escrow_including_the_one_unit_funding_target() {
    for supply in [1, 3] {
        let mut f = setup();
        let mut terms = params();
        terms.token_price = 1;
        terms.token_supply = supply;
        ok(
            &mut f.svm,
            create_ix(&f.operator.pubkey(), 0, terms),
            &f.operator,
        );
        ok(&mut f.svm, submit_ix(&f.operator.pubkey(), 0), &f.operator);
        ok(
            &mut f.svm,
            review_ix(&f.verifier.pubkey(), 0, 1, [1; 32], true),
            &f.verifier,
        );
        ok(
            &mut f.svm,
            activate_ix(&f.operator.pubkey(), 0, BOND),
            &f.operator,
        );
        ok(&mut f.svm, open_ix(&f.operator.pubkey(), 0), &f.operator);
        ok(
            &mut f.svm,
            reserve_ix(&f.buyer.pubkey(), 0, supply),
            &f.buyer,
        );
        ok(
            &mut f.svm,
            claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0, u64::MAX),
            &f.buyer,
        );
        assert_eq!(
            balance(&f.svm, ata(&f.operator.pubkey(), &payment_mint())),
            supply / 2
        );
        assert_eq!(balance(&f.svm, payment_vault(0)), supply - supply / 2);
        let settled = funding_of(&f.svm, 0);
        assert!(settled.first_tranche_released);
        assert_eq!(settled.first_tranche_amount, supply / 2);
    }
}

#[test]
fn an_already_funded_reservation_hub_can_release_once_to_its_recorded_operator() {
    let mut f = setup();
    funded_before_tranches(&mut f);
    assert!(f.svm.get_account(&funding(0)).is_none());
    f.svm.airdrop(&funding(0), 1).unwrap();
    ok(
        &mut f.svm,
        tranche_ix(&f.verifier.pubkey(), &f.operator.pubkey(), 0),
        &f.verifier,
    );
    assert_eq!(
        balance(&f.svm, ata(&f.operator.pubkey(), &payment_mint())),
        PRICE * SUPPLY / 2
    );
    assert_eq!(balance(&f.svm, payment_vault(0)), PRICE * SUPPLY / 2);
    let settled = funding_of(&f.svm, 0);
    assert_eq!(settled.hub, hub(0));
    assert!(settled.first_tranche_released);
    assert_eq!(settled.first_tranche_amount, PRICE * SUPPLY / 2);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    fails_with(
        &mut f.svm,
        tranche_ix(&f.buyer.pubkey(), &f.operator.pubkey(), 0),
        &f.buyer,
        "FirstTrancheAlreadyReleased",
    );
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
}

#[test]
fn insufficient_escrow_rolls_back_manual_tranche_account_initialization() {
    let mut f = setup();
    funded_before_tranches(&mut f);
    set_token_balance(&mut f.svm, payment_vault(0), PRICE * SUPPLY / 2 - 1);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    assert!(process(
        &mut f.svm,
        tranche_ix(&f.verifier.pubkey(), &f.operator.pubkey(), 0),
        &f.verifier
    )
    .is_err());
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    assert!(f.svm.get_account(&funding(0)).is_none());
    assert!(f
        .svm
        .get_account(&ata(&f.operator.pubkey(), &payment_mint()))
        .is_none());
}

#[test]
fn standalone_release_rejects_substituted_hub_accounts_and_recipients() {
    let mut f = setup();
    funded_before_tranches(&mut f);
    reach_listed(&mut f, 1);
    ok(&mut f.svm, open_ix(&f.operator.pubkey(), 1), &f.operator);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    for (index, wrong) in [
        (3, sale(1)),
        (5, payment_vault(1)),
        (6, f.buyer.pubkey()),
        (7, ata(&f.buyer.pubkey(), &payment_mint())),
        (8, funding(1)),
    ] {
        let mut ix = tranche_ix(&f.verifier.pubkey(), &f.operator.pubkey(), 0);
        ix.accounts[index].pubkey = wrong;
        assert!(process(&mut f.svm, ix, &f.verifier).is_err());
        assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    }
}

#[test]
fn the_new_tranche_instruction_does_not_release_legacy_paid_sale_escrow() {
    let mut f = setup();
    reach_listed(&mut f, 0);
    ok(
        &mut f.svm,
        buy_ix(&f.buyer.pubkey(), 0, SUPPLY, PRICE * SUPPLY),
        &f.buyer,
    );
    ok(&mut f.svm, common::claim_ix(&f.buyer.pubkey(), 0), &f.buyer);
    assert!(process(
        &mut f.svm,
        tranche_ix(&f.verifier.pubkey(), &f.operator.pubkey(), 0),
        &f.verifier
    )
    .is_err());
    assert_eq!(balance(&f.svm, payment_vault(0)), PRICE * SUPPLY);
    assert!(f.svm.get_account(&funding(0)).is_none());
}
