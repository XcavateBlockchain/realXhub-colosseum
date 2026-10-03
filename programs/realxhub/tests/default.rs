mod common;
use anchor_lang::solana_program::{clock::Clock, program_pack::Pack};
use anchor_spl::token::spl_token::state::{Account as SplAccount, Mint as SplMint};
use common::*;
use realxhub::state::{DefaultRedemption, HubDefault, HubFunding};
use realxhub::{AssessmentParams, EvidenceParams, MilestonePolicyParams};

const THRESHOLD: u16 = 7_000;
fn policy() -> Pubkey {
    address(&[realxhub::MILESTONE_POLICY_SEED])
}
fn funding(id: u64) -> Pubkey {
    address(&[realxhub::FUNDING_SEED, &id.to_le_bytes()])
}
fn milestone(id: u64) -> Pubkey {
    address(&[realxhub::MILESTONE_SEED, &id.to_le_bytes()])
}
fn sale(id: u64) -> Pubkey {
    address(&[realxhub::RESERVATION_SALE_SEED, &id.to_le_bytes()])
}
fn receipt(id: u64, buyer: &Pubkey) -> Pubkey {
    address(&[
        realxhub::RESERVATION_CLAIM_SEED,
        &id.to_le_bytes(),
        buyer.as_ref(),
    ])
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
fn policy_params(assessor: &Pubkey) -> MilestonePolicyParams {
    MilestonePolicyParams {
        assessor: *assessor,
        approval_threshold_bps: THRESHOLD,
        assessment_policy_uri: "https://example.com/assessment-policy.json".into(),
        assessment_policy_hash: [3; 32],
    }
}
fn policy_ix(authority: &Pubkey, params: MilestonePolicyParams) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::InitializeMilestonePolicy { params }.data(),
        realxhub::accounts::InitializeMilestonePolicy {
            authority: *authority,
            config: config(),
            policy: policy(),
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}
fn evidence(hash: u8) -> EvidenceParams {
    EvidenceParams {
        evidence_uri: "https://example.com/progress.json".into(),
        evidence_hash: [hash; 32],
    }
}
fn submit_ix(operator: &Pubkey, id: u64, params: EvidenceParams) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::SubmitEvidence { hub_id: id, params }.data(),
        realxhub::accounts::SubmitEvidence {
            operator: *operator,
            hub: hub(id),
            funding: funding(id),
            policy: policy(),
            milestone: milestone(id),
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}
fn assessment(revision: u32, hash: u8, score: u16) -> AssessmentParams {
    AssessmentParams {
        revision,
        evidence_hash: [hash; 32],
        assessment_policy_hash: [3; 32],
        probability_bps: score,
        assessment_uri: "https://example.com/assessment.json".into(),
        assessment_hash: [4; 32],
    }
}
fn assess_ix(
    assessor: &Pubkey,
    operator: &Pubkey,
    id: u64,
    params: AssessmentParams,
) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::AssessEvidence { hub_id: id, params }.data(),
        realxhub::accounts::AssessEvidence {
            assessor: *assessor,
            policy: policy(),
            hub: hub(id),
            funding: funding(id),
            milestone: milestone(id),
            config: config(),
            payment_mint: payment_mint(),
            payment_vault: payment_vault(id),
            operator: *operator,
            operator_payment: ata(operator, &payment_mint()),
            token_program: TOKEN,
            associated_token_program: ATA,
            system_program: SYS,
        }
        .to_account_metas(None),
    )
}

fn settlement(id: u64) -> Pubkey {
    address(&[realxhub::DEFAULT_SEED, &id.to_le_bytes()])
}
fn redemption(id: u64, buyer: &Pubkey) -> Pubkey {
    address(&[
        realxhub::DEFAULT_REDEMPTION_SEED,
        &id.to_le_bytes(),
        buyer.as_ref(),
    ])
}
fn declare_accounts(caller: &Pubkey, id: u64) -> realxhub::accounts::DeclareDefault {
    realxhub::accounts::DeclareDefault {
        cranker: *caller,
        config: config(),
        hub: hub(id),
        funding: funding(id),
        milestone: milestone(id),
        payment_mint: payment_mint(),
        xcav_mint: region_common::xcav_mint(),
        payment_vault: payment_vault(id),
        bond_vault: bond_vault(id),
        settlement: settlement(id),
        system_program: SYS,
    }
}
fn declare_ix(caller: &Pubkey, id: u64) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::DeclareDefault { hub_id: id }.data(),
        declare_accounts(caller, id).to_account_metas(None),
    )
}
fn redeem_accounts(buyer: &Pubkey, id: u64) -> realxhub::accounts::RedeemDefault {
    realxhub::accounts::RedeemDefault {
        buyer: *buyer,
        config: config(),
        hub: hub(id),
        settlement: settlement(id),
        claim: receipt(id, buyer),
        redemption: redemption(id, buyer),
        hub_mint: hub_mint(id),
        buyer_token: ata(buyer, &hub_mint(id)),
        payment_mint: payment_mint(),
        xcav_mint: region_common::xcav_mint(),
        payment_vault: payment_vault(id),
        bond_vault: bond_vault(id),
        buyer_payment: ata(buyer, &payment_mint()),
        buyer_xcav: ata(buyer, &region_common::xcav_mint()),
        token_program: TOKEN,
        associated_token_program: ATA,
        system_program: SYS,
    }
}
fn redeem_with(
    accounts: realxhub::accounts::RedeemDefault,
    id: u64,
    amount: u64,
    payment: u64,
    xcav: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::RedeemDefault {
            hub_id: id,
            amount,
            min_payment_out: payment,
            min_xcav_out: xcav,
        }
        .data(),
        accounts.to_account_metas(None),
    )
}
fn redeem_ix(buyer: &Pubkey, id: u64, amount: u64) -> Instruction {
    redeem_with(redeem_accounts(buyer, id), id, amount, 0, 0)
}
fn reserve_ix(buyer: &Pubkey, id: u64, amount: u64) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::ReserveTokens {
            hub_id: id,
            amount,
            max_total_cost: u64::MAX,
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
fn paid_claim_ix(operator: &Pubkey, buyer: &Pubkey, id: u64) -> Instruction {
    Instruction::new_with_bytes(
        realxhub::id(),
        &realxhub::instruction::ClaimReservedTokens {
            hub_id: id,
            max_total_cost: u64::MAX,
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
fn open_hub(f: &mut Fixture, id: u64, supply: u64, price: u64) {
    let mut terms = params();
    terms.token_supply = supply;
    terms.token_price = price;
    ok(
        &mut f.svm,
        create_ix(&f.operator.pubkey(), id, terms),
        &f.operator,
    );
    ok(
        &mut f.svm,
        common::submit_ix(&f.operator.pubkey(), id),
        &f.operator,
    );
    ok(
        &mut f.svm,
        review_ix(&f.verifier.pubkey(), id, 1, [1; 32], true),
        &f.verifier,
    );
    ok(
        &mut f.svm,
        activate_ix(&f.operator.pubkey(), id, BOND),
        &f.operator,
    );
    ok(
        &mut f.svm,
        Instruction::new_with_bytes(
            realxhub::id(),
            &realxhub::instruction::OpenReservations { hub_id: id }.data(),
            realxhub::accounts::OpenReservations {
                operator: f.operator.pubkey(),
                hub: hub(id),
                operator_role: region_common::role_pda(
                    &f.operator.pubkey(),
                    Role::RegionalOperator,
                ),
                region: region_common::region_pda(1),
                sale: sale(id),
                system_program: SYS,
            }
            .to_account_metas(None),
        ),
        &f.operator,
    );
}
fn funded_hub(f: &mut Fixture, id: u64, supply: u64, price: u64) {
    open_hub(f, id, supply, price);
    ok(
        &mut f.svm,
        reserve_ix(&f.buyer.pubkey(), id, supply),
        &f.buyer,
    );
    ok(
        &mut f.svm,
        paid_claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), id),
        &f.buyer,
    );
}
fn funding_of(svm: &LiteSVM, id: u64) -> HubFunding {
    HubFunding::try_deserialize(&mut &svm.get_account(&funding(id)).unwrap().data[..]).unwrap()
}
fn settlement_of(svm: &LiteSVM, id: u64) -> HubDefault {
    HubDefault::try_deserialize(&mut &svm.get_account(&settlement(id)).unwrap().data[..]).unwrap()
}
fn redeemed_of(svm: &LiteSVM, id: u64, buyer: &Pubkey) -> DefaultRedemption {
    DefaultRedemption::try_deserialize(
        &mut &svm.get_account(&redemption(id, buyer)).unwrap().data[..],
    )
    .unwrap()
}
fn set_time(svm: &mut LiteSVM, time: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = time;
    svm.set_sysvar(&clock);
}
fn expire(svm: &mut LiteSVM, id: u64) {
    let deadline =
        funding_of(svm, id).first_tranche_released_at + realxhub::MILESTONE_WINDOW_SECONDS;
    set_time(svm, deadline);
}
fn snapshot(svm: &LiteSVM, id: u64, buyer: &Pubkey) -> Vec<Option<region_common::Account>> {
    [
        hub(id),
        funding(id),
        milestone(id),
        settlement(id),
        redemption(id, buyer),
        receipt(id, buyer),
        hub_mint(id),
        payment_vault(id),
        bond_vault(id),
        ata(buyer, &hub_mint(id)),
        ata(buyer, &payment_mint()),
        ata(buyer, &region_common::xcav_mint()),
    ]
    .iter()
    .map(|key| svm.get_account(key))
    .collect()
}
fn set_token_balance(svm: &mut LiteSVM, key: Pubkey, amount: u64) {
    let mut account = svm.get_account(&key).unwrap();
    let mut token = SplAccount::unpack(&account.data).unwrap();
    token.amount = amount;
    token.pack_into_slice(&mut account.data);
    svm.set_account(key, account).unwrap();
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
fn token_transfer(svm: &mut LiteSVM, signer: &Keypair, mint: Pubkey, to: Pubkey, amount: u64) {
    let ix = anchor_spl::token::spl_token::instruction::transfer_checked(
        &TOKEN,
        &ata(&signer.pubkey(), &mint),
        &mint,
        &to,
        &signer.pubkey(),
        &[],
        amount,
        0,
    )
    .unwrap();
    ok(svm, ix, signer);
}

#[test]
fn default_opens_at_exact_deadline_without_evidence_or_policy() {
    let mut f = setup();
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    let deadline =
        funding_of(&f.svm, 0).first_tranche_released_at + realxhub::MILESTONE_WINDOW_SECONDS;
    set_time(&mut f.svm, deadline - 1);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    fails_with(
        &mut f.svm,
        declare_ix(&f.verifier.pubkey(), 0),
        &f.verifier,
        "DefaultStillActive",
    );
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    set_time(&mut f.svm, deadline);
    ok(&mut f.svm, declare_ix(&f.verifier.pubkey(), 0), &f.verifier);
    assert_eq!(hub_of(&f.svm, 0).status, HubStatus::Defaulted);
    let pool = settlement_of(&f.svm, 0);
    assert_eq!(pool.payment_pool, PRICE * SUPPLY / 2);
    assert_eq!(pool.xcav_pool, BOND);
    assert_eq!(pool.approval_deadline, deadline);
    assert_eq!(pool.defaulted_at, deadline);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    assert!(process(&mut f.svm, declare_ix(&f.verifier.pubkey(), 0), &f.verifier).is_err());
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
}

#[test]
fn original_buyer_burns_tokens_and_recovers_both_pools_once() {
    let mut f = setup();
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    expire(&mut f.svm, 0);
    ok(&mut f.svm, declare_ix(&f.verifier.pubkey(), 0), &f.verifier);
    let before = balance(&f.svm, ata(&f.buyer.pubkey(), &payment_mint()));
    ok(
        &mut f.svm,
        redeem_ix(&f.buyer.pubkey(), 0, SUPPLY),
        &f.buyer,
    );
    assert_eq!(
        balance(&f.svm, ata(&f.buyer.pubkey(), &payment_mint())),
        before + PRICE * SUPPLY / 2
    );
    assert_eq!(
        balance(&f.svm, ata(&f.buyer.pubkey(), &region_common::xcav_mint())),
        BOND
    );
    assert_eq!(balance(&f.svm, payment_vault(0)), 0);
    assert_eq!(balance(&f.svm, bond_vault(0)), 0);
    assert_eq!(balance(&f.svm, ata(&f.buyer.pubkey(), &hub_mint(0))), 0);
    assert_eq!(
        SplMint::unpack(&f.svm.get_account(&hub_mint(0)).unwrap().data)
            .unwrap()
            .supply,
        0
    );
    let pool = settlement_of(&f.svm, 0);
    assert_eq!(pool.tokens_redeemed, SUPPLY);
    assert_eq!(pool.payment_redeemed, pool.payment_pool);
    assert_eq!(pool.xcav_redeemed, BOND);
    assert_eq!(redeemed_of(&f.svm, 0, &f.buyer.pubkey()).amount, SUPPLY);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    fails_with(
        &mut f.svm,
        redeem_ix(&f.buyer.pubkey(), 0, 1),
        &f.buyer,
        "RedemptionLimitExceeded",
    );
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    assert!(process(
        &mut f.svm,
        bond_refund_ix(&f.operator.pubkey(), 0),
        &f.operator
    )
    .is_err());
}

#[test]
fn rejected_evidence_can_default_but_approved_evidence_cannot() {
    for score in [THRESHOLD - 1, THRESHOLD] {
        let mut f = setup();
        let assessor = region_common::funded(&mut f.svm);
        ok(
            &mut f.svm,
            policy_ix(&f.authority.pubkey(), policy_params(&assessor.pubkey())),
            &f.authority,
        );
        funded_hub(&mut f, 0, SUPPLY, PRICE);
        ok(
            &mut f.svm,
            submit_ix(&f.operator.pubkey(), 0, evidence(2)),
            &f.operator,
        );
        ok(
            &mut f.svm,
            assess_ix(
                &assessor.pubkey(),
                &f.operator.pubkey(),
                0,
                assessment(1, 2, score),
            ),
            &assessor,
        );
        expire(&mut f.svm, 0);
        if score == THRESHOLD {
            let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
            fails_with(
                &mut f.svm,
                declare_ix(&f.buyer.pubkey(), 0),
                &f.buyer,
                "DefaultNotAllowed",
            );
            assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
        } else {
            ok(&mut f.svm, declare_ix(&f.buyer.pubkey(), 0), &f.buyer);
            assert!(process(
                &mut f.svm,
                submit_ix(&f.operator.pubkey(), 0, evidence(5)),
                &f.operator
            )
            .is_err());
            assert!(process(
                &mut f.svm,
                assess_ix(
                    &assessor.pubkey(),
                    &f.operator.pubkey(),
                    0,
                    assessment(1, 2, THRESHOLD)
                ),
                &assessor
            )
            .is_err());
        }
    }
}

#[test]
fn pending_evidence_and_absent_prefunded_milestone_allow_default() {
    for submitted in [true, false] {
        let mut f = setup();
        let assessor = region_common::funded(&mut f.svm);
        ok(
            &mut f.svm,
            policy_ix(&f.authority.pubkey(), policy_params(&assessor.pubkey())),
            &f.authority,
        );
        funded_hub(&mut f, 0, SUPPLY, PRICE);
        if submitted {
            ok(
                &mut f.svm,
                submit_ix(&f.operator.pubkey(), 0, evidence(2)),
                &f.operator,
            );
        } else {
            ok(
                &mut f.svm,
                anchor_lang::solana_program::system_instruction::transfer(
                    &f.buyer.pubkey(),
                    &milestone(0),
                    1_000_000,
                ),
                &f.buyer,
            );
        }
        expire(&mut f.svm, 0);
        ok(&mut f.svm, declare_ix(&f.buyer.pubkey(), 0), &f.buyer);
        assert_eq!(hub_of(&f.svm, 0).status, HubStatus::Defaulted);
    }
}

#[test]
fn donations_do_not_enlarge_default_pools_or_buyer_payouts() {
    let mut f = setup();
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    set_token_balance(&mut f.svm, payment_vault(0), PRICE * SUPPLY / 2 + 17);
    set_token_balance(&mut f.svm, bond_vault(0), BOND + 31);
    expire(&mut f.svm, 0);
    ok(&mut f.svm, declare_ix(&f.buyer.pubkey(), 0), &f.buyer);
    let pool = settlement_of(&f.svm, 0);
    assert_eq!(pool.payment_pool, PRICE * SUPPLY / 2);
    assert_eq!(pool.xcav_pool, BOND);
    ok(
        &mut f.svm,
        redeem_ix(&f.buyer.pubkey(), 0, SUPPLY),
        &f.buyer,
    );
    assert_eq!(balance(&f.svm, payment_vault(0)), 17);
    assert_eq!(balance(&f.svm, bond_vault(0)), 31);
}

#[test]
fn cumulative_rounding_and_final_dust_pay_each_pool_exactly() {
    let mut f = setup();
    let second = investor(&mut f);
    let last = investor(&mut f);
    open_hub(&mut f, 0, 6, 1);
    for (buyer, amount) in [(&f.buyer, 2), (&second, 3), (&last, 1)] {
        ok(&mut f.svm, reserve_ix(&buyer.pubkey(), 0, amount), buyer);
    }
    for buyer in [&f.buyer, &second, &last] {
        ok(
            &mut f.svm,
            paid_claim_ix(&f.operator.pubkey(), &buyer.pubkey(), 0),
            buyer,
        );
    }
    expire(&mut f.svm, 0);
    ok(&mut f.svm, declare_ix(&f.verifier.pubkey(), 0), &f.verifier);
    ok(&mut f.svm, redeem_ix(&f.buyer.pubkey(), 0, 1), &f.buyer);
    assert_eq!(
        redeemed_of(&f.svm, 0, &f.buyer.pubkey()).payment_received,
        0
    );
    ok(&mut f.svm, redeem_ix(&f.buyer.pubkey(), 0, 1), &f.buyer);
    let a = redeemed_of(&f.svm, 0, &f.buyer.pubkey());
    assert_eq!(a.payment_received, 1);
    assert_eq!(a.xcav_received, BOND / 3);
    ok(&mut f.svm, redeem_ix(&second.pubkey(), 0, 3), &second);
    ok(&mut f.svm, redeem_ix(&last.pubkey(), 0, 1), &last);
    let pool = settlement_of(&f.svm, 0);
    assert_eq!(pool.tokens_redeemed, 6);
    assert_eq!(pool.payment_redeemed, 3);
    assert_eq!(pool.xcav_redeemed, BOND);
    assert_eq!(balance(&f.svm, payment_vault(0)), 0);
    assert_eq!(balance(&f.svm, bond_vault(0)), 0);
    assert_eq!(redeemed_of(&f.svm, 0, &last.pubkey()).payment_received, 1);
    assert_eq!(
        redeemed_of(&f.svm, 0, &last.pubkey()).xcav_received,
        BOND - BOND / 3 - BOND / 2
    );
}

#[test]
fn secondary_holder_cannot_redeem_and_original_buyer_must_surrender_tokens() {
    let mut f = setup();
    let holder = investor(&mut f);
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    let ata_ix =
        anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &holder.pubkey(),
            &holder.pubkey(),
            &hub_mint(0),
            &TOKEN,
        );
    ok(&mut f.svm, ata_ix, &holder);
    token_transfer(
        &mut f.svm,
        &f.buyer,
        hub_mint(0),
        ata(&holder.pubkey(), &hub_mint(0)),
        SUPPLY,
    );
    expire(&mut f.svm, 0);
    ok(&mut f.svm, declare_ix(&f.verifier.pubkey(), 0), &f.verifier);
    assert!(process(&mut f.svm, redeem_ix(&holder.pubkey(), 0, SUPPLY), &holder).is_err());
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    fails_with(
        &mut f.svm,
        redeem_ix(&f.buyer.pubkey(), 0, SUPPLY),
        &f.buyer,
        "InsufficientRedemptionTokens",
    );
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    token_transfer(
        &mut f.svm,
        &holder,
        hub_mint(0),
        ata(&f.buyer.pubkey(), &hub_mint(0)),
        SUPPLY,
    );
    ok(
        &mut f.svm,
        redeem_ix(&f.buyer.pubkey(), 0, SUPPLY),
        &f.buyer,
    );
}

#[test]
fn original_buyer_cannot_redeem_another_buyers_allocation() {
    let mut f = setup();
    let other = investor(&mut f);
    open_hub(&mut f, 0, SUPPLY, PRICE);
    ok(&mut f.svm, reserve_ix(&f.buyer.pubkey(), 0, 4), &f.buyer);
    ok(&mut f.svm, reserve_ix(&other.pubkey(), 0, 6), &other);
    ok(
        &mut f.svm,
        paid_claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0),
        &f.buyer,
    );
    ok(
        &mut f.svm,
        paid_claim_ix(&f.operator.pubkey(), &other.pubkey(), 0),
        &other,
    );
    token_transfer(
        &mut f.svm,
        &other,
        hub_mint(0),
        ata(&f.buyer.pubkey(), &hub_mint(0)),
        6,
    );
    expire(&mut f.svm, 0);
    ok(&mut f.svm, declare_ix(&f.verifier.pubkey(), 0), &f.verifier);
    fails_with(
        &mut f.svm,
        redeem_ix(&f.buyer.pubkey(), 0, 5),
        &f.buyer,
        "RedemptionLimitExceeded",
    );
    ok(&mut f.svm, redeem_ix(&f.buyer.pubkey(), 0, 4), &f.buyer);
    fails_with(
        &mut f.svm,
        redeem_ix(&f.buyer.pubkey(), 0, 1),
        &f.buyer,
        "RedemptionLimitExceeded",
    );
}

#[test]
fn redemption_does_not_require_continued_compliance() {
    let mut f = setup();
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    expire(&mut f.svm, 0);
    ok(&mut f.svm, declare_ix(&f.buyer.pubkey(), 0), &f.buyer);
    region_common::set_permission(
        &mut f.svm,
        &f.authority,
        &f.buyer.pubkey(),
        Role::RealEstateInvestor,
        false,
    );
    ok(
        &mut f.svm,
        redeem_ix(&f.buyer.pubkey(), 0, SUPPLY),
        &f.buyer,
    );
}

#[test]
fn minimum_payout_zero_quantity_and_over_redemption_leave_assets_unchanged() {
    let mut f = setup();
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    expire(&mut f.svm, 0);
    ok(&mut f.svm, declare_ix(&f.buyer.pubkey(), 0), &f.buyer);
    for (quantity, payment, xcav, error) in [
        (0, 0, 0, "InvalidAmount"),
        (SUPPLY + 1, 0, 0, "RedemptionLimitExceeded"),
        (SUPPLY, PRICE * SUPPLY / 2 + 1, 0, "PayoutBelowMinimum"),
        (SUPPLY, 0, BOND + 1, "PayoutBelowMinimum"),
    ] {
        let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
        fails_with(
            &mut f.svm,
            redeem_with(
                redeem_accounts(&f.buyer.pubkey(), 0),
                0,
                quantity,
                payment,
                xcav,
            ),
            &f.buyer,
            error,
        );
        assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    }
}

#[test]
fn second_asset_transfer_failure_rolls_back_burn_first_payment_and_receipt() {
    let mut f = setup();
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    expire(&mut f.svm, 0);
    ok(&mut f.svm, declare_ix(&f.buyer.pubkey(), 0), &f.buyer);
    // Fault injection after declaration forces the XCAV transfer to fail after
    // a successful burn and stablecoin transfer, proving transaction rollback.
    set_token_balance(&mut f.svm, bond_vault(0), BOND - 1);
    let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
    assert!(process(
        &mut f.svm,
        redeem_ix(&f.buyer.pubkey(), 0, SUPPLY),
        &f.buyer
    )
    .is_err());
    assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    set_token_balance(&mut f.svm, bond_vault(0), BOND);
    ok(
        &mut f.svm,
        redeem_ix(&f.buyer.pubkey(), 0, SUPPLY),
        &f.buyer,
    );
}

#[test]
fn substituted_milestone_receipt_vaults_mints_and_recipients_cannot_receive_funds() {
    let mut f = setup();
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    expire(&mut f.svm, 0);
    let mut declare = declare_accounts(&f.buyer.pubkey(), 0);
    declare.milestone = milestone(1);
    assert!(process(
        &mut f.svm,
        Instruction::new_with_bytes(
            realxhub::id(),
            &realxhub::instruction::DeclareDefault { hub_id: 0 }.data(),
            declare.to_account_metas(None)
        ),
        &f.buyer
    )
    .is_err());
    ok(&mut f.svm, declare_ix(&f.buyer.pubkey(), 0), &f.buyer);
    for field in 0..8 {
        let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
        let mut accounts = redeem_accounts(&f.buyer.pubkey(), 0);
        match field {
            0 => accounts.claim = receipt(0, &f.operator.pubkey()),
            1 => accounts.settlement = settlement(1),
            2 => accounts.payment_vault = bond_vault(0),
            3 => accounts.bond_vault = payment_vault(0),
            4 => accounts.hub_mint = payment_mint(),
            5 => accounts.payment_mint = region_common::xcav_mint(),
            6 => accounts.buyer_payment = ata(&f.operator.pubkey(), &payment_mint()),
            _ => accounts.buyer_xcav = ata(&f.operator.pubkey(), &region_common::xcav_mint()),
        }
        assert!(process(&mut f.svm, redeem_with(accounts, 0, SUPPLY, 0, 0), &f.buyer).is_err());
        assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
    }
}

#[test]
fn default_requires_full_funding_and_sufficient_recorded_escrow() {
    let mut f = setup();
    open_hub(&mut f, 0, SUPPLY, PRICE);
    ok(&mut f.svm, reserve_ix(&f.buyer.pubkey(), 0, 4), &f.buyer);
    let other = investor(&mut f);
    ok(&mut f.svm, reserve_ix(&other.pubkey(), 0, 6), &other);
    ok(
        &mut f.svm,
        paid_claim_ix(&f.operator.pubkey(), &f.buyer.pubkey(), 0),
        &f.buyer,
    );
    set_time(&mut f.svm, 10_000_000);
    assert!(process(&mut f.svm, declare_ix(&f.buyer.pubkey(), 0), &f.buyer).is_err());
    let mut f = setup();
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    expire(&mut f.svm, 0);
    for (vault, expected) in [
        (payment_vault(0), PRICE * SUPPLY / 2),
        (bond_vault(0), BOND),
    ] {
        set_token_balance(&mut f.svm, vault, expected - 1);
        let before = snapshot(&f.svm, 0, &f.buyer.pubkey());
        fails_with(
            &mut f.svm,
            declare_ix(&f.buyer.pubkey(), 0),
            &f.buyer,
            "InsufficientDefaultEscrow",
        );
        assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before);
        set_token_balance(&mut f.svm, vault, expected);
    }
}

#[test]
fn one_unit_target_recovers_its_zero_first_tranche_remainder() {
    let mut f = setup();
    funded_hub(&mut f, 0, 1, 1);
    assert_eq!(funding_of(&f.svm, 0).first_tranche_amount, 0);
    expire(&mut f.svm, 0);
    ok(&mut f.svm, declare_ix(&f.buyer.pubkey(), 0), &f.buyer);
    let before = balance(&f.svm, ata(&f.buyer.pubkey(), &payment_mint()));
    ok(
        &mut f.svm,
        redeem_with(redeem_accounts(&f.buyer.pubkey(), 0), 0, 1, 1, BOND),
        &f.buyer,
    );
    assert_eq!(
        balance(&f.svm, ata(&f.buyer.pubkey(), &payment_mint())),
        before + 1
    );
    assert_eq!(balance(&f.svm, payment_vault(0)), 0);
    assert_eq!(balance(&f.svm, bond_vault(0)), 0);
}

#[test]
fn valid_records_from_another_defaulted_hub_cannot_be_substituted() {
    let mut f = setup();
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    funded_hub(&mut f, 1, SUPPLY, PRICE);
    expire(&mut f.svm, 1);
    ok(&mut f.svm, declare_ix(&f.verifier.pubkey(), 0), &f.verifier);
    ok(&mut f.svm, declare_ix(&f.verifier.pubkey(), 1), &f.verifier);
    ok(&mut f.svm, redeem_ix(&f.buyer.pubkey(), 1, 1), &f.buyer);
    for field in 0..5 {
        let before_zero = snapshot(&f.svm, 0, &f.buyer.pubkey());
        let before_one = snapshot(&f.svm, 1, &f.buyer.pubkey());
        let mut accounts = redeem_accounts(&f.buyer.pubkey(), 0);
        match field {
            0 => accounts.claim = receipt(1, &f.buyer.pubkey()),
            1 => accounts.redemption = redemption(1, &f.buyer.pubkey()),
            2 => accounts.settlement = settlement(1),
            3 => accounts.payment_vault = payment_vault(1),
            _ => accounts.bond_vault = bond_vault(1),
        }
        assert!(process(&mut f.svm, redeem_with(accounts, 0, 1, 0, 0), &f.buyer).is_err());
        assert_eq!(snapshot(&f.svm, 0, &f.buyer.pubkey()), before_zero);
        assert_eq!(snapshot(&f.svm, 1, &f.buyer.pubkey()), before_one);
    }
}
