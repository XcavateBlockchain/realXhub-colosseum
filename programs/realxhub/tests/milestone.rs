mod common;
use anchor_lang::solana_program::{clock::Clock, program_pack::Pack};
use anchor_spl::token::spl_token::state::{Account as SplAccount, AccountState};
use common::*;
use realxhub::state::{EvidenceStatus, HubFunding, HubMilestone, MilestonePolicy};
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
fn funded_hub(f: &mut Fixture, id: u64, supply: u64, price: u64) {
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
    ok(
        &mut f.svm,
        Instruction::new_with_bytes(
            realxhub::id(),
            &realxhub::instruction::ReserveTokens {
                hub_id: id,
                amount: supply,
                max_total_cost: u64::MAX,
            }
            .data(),
            realxhub::accounts::ReserveTokens {
                buyer: f.buyer.pubkey(),
                config: config(),
                hub: hub(id),
                buyer_role: region_common::role_pda(&f.buyer.pubkey(), Role::RealEstateInvestor),
                sale: sale(id),
                payment_mint: payment_mint(),
                buyer_payment: ata(&f.buyer.pubkey(), &payment_mint()),
                reservation: reservation(id, &f.buyer.pubkey()),
                payment_reservation: ledger(&f.buyer.pubkey()),
                system_program: SYS,
            }
            .to_account_metas(None),
        ),
        &f.buyer,
    );
    ok(
        &mut f.svm,
        Instruction::new_with_bytes(
            realxhub::id(),
            &realxhub::instruction::ClaimReservedTokens {
                hub_id: id,
                max_total_cost: u64::MAX,
            }
            .data(),
            realxhub::accounts::ClaimReservedTokens {
                buyer: f.buyer.pubkey(),
                config: config(),
                hub: hub(id),
                buyer_role: region_common::role_pda(&f.buyer.pubkey(), Role::RealEstateInvestor),
                sale: sale(id),
                reservation: reservation(id, &f.buyer.pubkey()),
                payment_reservation: ledger(&f.buyer.pubkey()),
                payment_mint: payment_mint(),
                buyer_payment: ata(&f.buyer.pubkey(), &payment_mint()),
                payment_vault: payment_vault(id),
                hub_mint: hub_mint(id),
                token_vault: token_vault(id),
                buyer_token: ata(&f.buyer.pubkey(), &hub_mint(id)),
                records: realxhub::accounts::ClaimRecords {
                    buyer: f.buyer.pubkey(),
                    hub: hub(id),
                    claim: receipt(id, &f.buyer.pubkey()),
                    funding: funding(id),
                    system_program: SYS,
                },
                operator: f.operator.pubkey(),
                operator_payment: ata(&f.operator.pubkey(), &payment_mint()),
                token_program: TOKEN,
                associated_token_program: ATA,
                system_program: SYS,
            }
            .to_account_metas(None),
        ),
        &f.buyer,
    );
}
fn scenario() -> (Fixture, Keypair) {
    let mut f = setup();
    let assessor = region_common::funded(&mut f.svm);
    ok(
        &mut f.svm,
        policy_ix(&f.authority.pubkey(), policy_params(&assessor.pubkey())),
        &f.authority,
    );
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    (f, assessor)
}
fn milestone_of(svm: &LiteSVM, id: u64) -> HubMilestone {
    HubMilestone::try_deserialize(&mut &svm.get_account(&milestone(id)).unwrap().data[..]).unwrap()
}
fn funding_of(svm: &LiteSVM, id: u64) -> HubFunding {
    HubFunding::try_deserialize(&mut &svm.get_account(&funding(id)).unwrap().data[..]).unwrap()
}
fn snapshot(f: &Fixture, id: u64) -> Vec<Option<region_common::Account>> {
    [
        hub(id),
        funding(id),
        milestone(id),
        payment_vault(id),
        bond_vault(id),
        ata(&f.operator.pubkey(), &payment_mint()),
        ata(&f.buyer.pubkey(), &hub_mint(id)),
    ]
    .iter()
    .map(|key| f.svm.get_account(key))
    .collect()
}
fn set_time(svm: &mut LiteSVM, time: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = time;
    svm.set_sysvar(&clock);
}

#[test]
fn policy_is_authority_initialized_and_immutable_with_no_default_threshold() {
    let mut f = setup();
    let assessor = region_common::funded(&mut f.svm);
    fails_with(
        &mut f.svm,
        policy_ix(&f.buyer.pubkey(), policy_params(&assessor.pubkey())),
        &f.buyer,
        "NotUpgradeAuthority",
    );
    for bad in [0, 10_001] {
        let mut p = policy_params(&assessor.pubkey());
        p.approval_threshold_bps = bad;
        fails_with(
            &mut f.svm,
            policy_ix(&f.authority.pubkey(), p),
            &f.authority,
            "InvalidMilestonePolicy",
        );
        assert!(f.svm.get_account(&policy()).is_none());
    }
    for (key, uri, hash) in [
        (Pubkey::default(), "policy", [3; 32]),
        (assessor.pubkey(), "   ", [3; 32]),
        (assessor.pubkey(), "policy", [0; 32]),
    ] {
        let mut p = policy_params(&key);
        p.assessment_policy_uri = uri.into();
        p.assessment_policy_hash = hash;
        fails_with(
            &mut f.svm,
            policy_ix(&f.authority.pubkey(), p),
            &f.authority,
            "InvalidMilestonePolicy",
        );
    }
    f.svm.airdrop(&policy(), 1).unwrap();
    ok(
        &mut f.svm,
        policy_ix(&f.authority.pubkey(), policy_params(&assessor.pubkey())),
        &f.authority,
    );
    let before = f.svm.get_account(&policy());
    let mut p = policy_params(&f.buyer.pubkey());
    p.approval_threshold_bps = 1;
    assert!(process(
        &mut f.svm,
        policy_ix(&f.authority.pubkey(), p),
        &f.authority
    )
    .is_err());
    assert_eq!(f.svm.get_account(&policy()), before);
    let stored = MilestonePolicy::try_deserialize(&mut &before.unwrap().data[..]).unwrap();
    assert_eq!(stored.approval_threshold_bps, THRESHOLD);
}

#[test]
fn approval_at_threshold_releases_exact_second_half_and_preserves_bond_and_tokens() {
    let (mut f, assessor) = scenario();
    f.svm.airdrop(&milestone(0), 1).unwrap();
    let first = f.svm.get_account(&funding(0));
    let tokens = f.svm.get_account(&ata(&f.buyer.pubkey(), &hub_mint(0)));
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator,
    );
    let pending = milestone_of(&f.svm, 0);
    assert_eq!(
        pending.deadline,
        funding_of(&f.svm, 0).first_tranche_released_at + realxhub::MILESTONE_WINDOW_SECONDS
    );
    assert_eq!(pending.status, EvidenceStatus::Submitted);
    assert_eq!(balance(&f.svm, payment_vault(0)), PRICE * SUPPLY / 2);
    ok(
        &mut f.svm,
        assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(1, 2, THRESHOLD),
        ),
        &assessor,
    );
    let paid = milestone_of(&f.svm, 0);
    assert_eq!(paid.status, EvidenceStatus::Approved);
    assert_eq!(paid.probability_bps, THRESHOLD);
    assert!(paid.second_tranche_released);
    assert_eq!(paid.second_tranche_amount, PRICE * SUPPLY / 2);
    assert_eq!(
        paid.second_tranche_released_at,
        f.svm.get_sysvar::<Clock>().unix_timestamp
    );
    assert_eq!(paid.assessment_hash, [4; 32]);
    assert_eq!(balance(&f.svm, payment_vault(0)), 0);
    assert_eq!(
        balance(&f.svm, ata(&f.operator.pubkey(), &payment_mint())),
        PRICE * SUPPLY
    );
    assert_eq!(balance(&f.svm, bond_vault(0)), BOND);
    assert_eq!(f.svm.get_account(&funding(0)), first);
    assert_eq!(
        f.svm.get_account(&ata(&f.buyer.pubkey(), &hub_mint(0))),
        tokens
    );
    assert_eq!(hub_of(&f.svm, 0).status, HubStatus::Funded);
}

#[test]
fn low_probability_rejects_without_payout_and_new_evidence_can_be_assessed() {
    let (mut f, assessor) = scenario();
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator,
    );
    let vault = f.svm.get_account(&payment_vault(0));
    ok(
        &mut f.svm,
        assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(1, 2, THRESHOLD - 1),
        ),
        &assessor,
    );
    let rejected = milestone_of(&f.svm, 0);
    assert_eq!(rejected.status, EvidenceStatus::Rejected);
    assert!(!rejected.second_tranche_released);
    assert_eq!(f.svm.get_account(&payment_vault(0)), vault);
    let deadline = rejected.deadline;
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(5)),
        &f.operator,
    );
    let revised = milestone_of(&f.svm, 0);
    assert_eq!(revised.revision, 2);
    assert_eq!(revised.deadline, deadline);
    assert_eq!(revised.assessment_hash, [0; 32]);
    assert_eq!(revised.assessed_at, 0);
    ok(
        &mut f.svm,
        assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(2, 5, 10_000),
        ),
        &assessor,
    );
    assert!(milestone_of(&f.svm, 0).second_tranche_released);
}

#[test]
fn stale_evidence_revision_hash_and_methodology_cannot_authorize_payment() {
    let (mut f, assessor) = scenario();
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator,
    );
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(5)),
        &f.operator,
    );
    let before = snapshot(&f, 0);
    let mut wrong_policy = assessment(2, 5, THRESHOLD);
    wrong_policy.assessment_policy_hash = [9; 32];
    for p in [
        assessment(1, 2, THRESHOLD),
        assessment(1, 5, THRESHOLD),
        assessment(2, 2, THRESHOLD),
        wrong_policy,
    ] {
        fails_with(
            &mut f.svm,
            assess_ix(&assessor.pubkey(), &f.operator.pubkey(), 0, p),
            &assessor,
            "EvidenceMismatch",
        );
        assert_eq!(snapshot(&f, 0), before);
    }
}

#[test]
fn only_the_recorded_operator_can_submit_and_only_the_assessor_can_score() {
    let (mut f, assessor) = scenario();
    let before = snapshot(&f, 0);
    fails_with(
        &mut f.svm,
        submit_ix(&f.buyer.pubkey(), 0, evidence(2)),
        &f.buyer,
        "NotOperator",
    );
    assert_eq!(snapshot(&f, 0), before);
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator,
    );
    let before = snapshot(&f, 0);
    for signer in [&f.operator, &f.buyer, &f.verifier, &f.authority] {
        fails_with(
            &mut f.svm,
            assess_ix(
                &signer.pubkey(),
                &f.operator.pubkey(),
                0,
                assessment(1, 2, THRESHOLD),
            ),
            signer,
            "NotAssessor",
        );
        assert_eq!(snapshot(&f, 0), before);
    }
    ok(
        &mut f.svm,
        assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(1, 2, THRESHOLD),
        ),
        &assessor,
    );
}

#[test]
fn operator_cannot_be_the_assessor_for_their_own_hub() {
    let mut f = setup();
    ok(
        &mut f.svm,
        policy_ix(&f.authority.pubkey(), policy_params(&f.operator.pubkey())),
        &f.authority,
    );
    funded_hub(&mut f, 0, SUPPLY, PRICE);
    let before = snapshot(&f, 0);
    fails_with(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator,
        "SelfReview",
    );
    assert_eq!(snapshot(&f, 0), before);
}

#[test]
fn paid_tranches_and_evidence_cannot_be_replayed() {
    let (mut f, assessor) = scenario();
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
            assessment(1, 2, THRESHOLD),
        ),
        &assessor,
    );
    let before = snapshot(&f, 0);
    fails_with(
        &mut f.svm,
        assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(1, 2, THRESHOLD),
        ),
        &assessor,
        "SecondTrancheAlreadyReleased",
    );
    fails_with(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(5)),
        &f.operator,
        "SecondTrancheAlreadyReleased",
    );
    assert_eq!(snapshot(&f, 0), before);
}

#[test]
fn rejection_cannot_be_changed_without_a_new_operator_submission() {
    let (mut f, assessor) = scenario();
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
            assessment(1, 2, 0),
        ),
        &assessor,
    );
    let before = snapshot(&f, 0);
    fails_with(
        &mut f.svm,
        assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(1, 2, 10_000),
        ),
        &assessor,
        "InvalidStatus",
    );
    assert_eq!(snapshot(&f, 0), before);
}

#[test]
fn malformed_references_and_out_of_range_scores_leave_funds_untouched() {
    let (mut f, assessor) = scenario();
    let before = snapshot(&f, 0);
    for p in [
        EvidenceParams {
            evidence_uri: " ".into(),
            evidence_hash: [2; 32],
        },
        EvidenceParams {
            evidence_uri: "x".repeat(201),
            evidence_hash: [2; 32],
        },
        evidence(0),
    ] {
        fails_with(
            &mut f.svm,
            submit_ix(&f.operator.pubkey(), 0, p),
            &f.operator,
            "InvalidEvidence",
        );
        assert_eq!(snapshot(&f, 0), before);
    }
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator,
    );
    let before = snapshot(&f, 0);
    fails_with(
        &mut f.svm,
        assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(1, 2, 10_001),
        ),
        &assessor,
        "InvalidProbability",
    );
    for (uri, hash) in [
        ("".into(), [4; 32]),
        ("x".repeat(201), [4; 32]),
        ("report".into(), [0; 32]),
    ] {
        let mut p = assessment(1, 2, THRESHOLD);
        p.assessment_uri = uri;
        p.assessment_hash = hash;
        fails_with(
            &mut f.svm,
            assess_ix(&assessor.pubkey(), &f.operator.pubkey(), 0, p),
            &assessor,
            "InvalidEvidence",
        );
        assert_eq!(snapshot(&f, 0), before);
    }
}

#[test]
fn approval_deadline_is_exclusive_and_submission_does_not_extend_it() {
    let (mut f, assessor) = scenario();
    let deadline =
        funding_of(&f.svm, 0).first_tranche_released_at + realxhub::MILESTONE_WINDOW_SECONDS;
    set_time(&mut f.svm, deadline - 1);
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator,
    );
    assert_eq!(milestone_of(&f.svm, 0).deadline, deadline);
    set_time(&mut f.svm, deadline);
    let before = snapshot(&f, 0);
    fails_with(
        &mut f.svm,
        assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(1, 2, THRESHOLD),
        ),
        &assessor,
        "MilestoneExpired",
    );
    fails_with(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(5)),
        &f.operator,
        "MilestoneExpired",
    );
    assert_eq!(snapshot(&f, 0), before);
    assert_eq!(balance(&f.svm, bond_vault(0)), BOND);
    assert!(process(
        &mut f.svm,
        bond_refund_ix(&f.operator.pubkey(), 0),
        &f.operator
    )
    .is_err());
}

#[test]
fn approval_one_second_before_deadline_is_allowed() {
    let (mut f, assessor) = scenario();
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator,
    );
    let deadline = milestone_of(&f.svm, 0).deadline;
    set_time(&mut f.svm, deadline - 1);
    ok(
        &mut f.svm,
        assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(1, 2, THRESHOLD),
        ),
        &assessor,
    );
    assert!(milestone_of(&f.svm, 0).second_tranche_released);
}

#[test]
fn evidence_requires_completed_claims_and_released_first_tranche() {
    let mut f = setup();
    let assessor = region_common::funded(&mut f.svm);
    ok(
        &mut f.svm,
        policy_ix(&f.authority.pubkey(), policy_params(&assessor.pubkey())),
        &f.authority,
    );
    reach_listed(&mut f, 0);
    let hub_before = f.svm.get_account(&hub(0));
    assert!(process(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator
    )
    .is_err());
    assert!(f.svm.get_account(&milestone(0)).is_none());
    assert_eq!(f.svm.get_account(&hub(0)), hub_before);
}

#[test]
fn another_hubs_funding_milestone_vault_or_recipient_cannot_be_substituted() {
    let (mut f, assessor) = scenario();
    funded_hub(&mut f, 1, SUPPLY, PRICE);
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator,
    );
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 1, evidence(2)),
        &f.operator,
    );
    let before = snapshot(&f, 0);
    let other_before = snapshot(&f, 1);
    for (index, wrong) in [
        (3, funding(1)),
        (4, milestone(1)),
        (7, payment_vault(1)),
        (8, f.buyer.pubkey()),
        (9, ata(&f.buyer.pubkey(), &payment_mint())),
    ] {
        let mut ix = assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(1, 2, THRESHOLD),
        );
        ix.accounts[index].pubkey = wrong;
        assert!(process(&mut f.svm, ix, &assessor).is_err());
        assert_eq!(snapshot(&f, 0), before);
        assert_eq!(snapshot(&f, 1), other_before);
    }
}

#[test]
fn failed_payout_rolls_back_approval_and_can_be_retried() {
    let (mut f, assessor) = scenario();
    ok(
        &mut f.svm,
        submit_ix(&f.operator.pubkey(), 0, evidence(2)),
        &f.operator,
    );
    let key = ata(&f.operator.pubkey(), &payment_mint());
    let mut account = f.svm.get_account(&key).unwrap();
    let mut token = SplAccount::unpack(&account.data).unwrap();
    // Fault injection: classic payment mints normally have no freeze authority.
    token.state = AccountState::Frozen;
    token.pack_into_slice(&mut account.data);
    f.svm.set_account(key, account).unwrap();
    let before = snapshot(&f, 0);
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
    assert_eq!(snapshot(&f, 0), before);
    assert_eq!(milestone_of(&f.svm, 0).status, EvidenceStatus::Submitted);
    let mut account = f.svm.get_account(&key).unwrap();
    token.state = AccountState::Initialized;
    token.pack_into_slice(&mut account.data);
    f.svm.set_account(key, account).unwrap();
    ok(
        &mut f.svm,
        assess_ix(
            &assessor.pubkey(),
            &f.operator.pubkey(),
            0,
            assessment(1, 2, THRESHOLD),
        ),
        &assessor,
    );
}

#[test]
fn odd_payment_remainders_are_paid_but_unsolicited_donations_stay_in_escrow() {
    for supply in [1, 3] {
        let mut f = setup();
        let assessor = region_common::funded(&mut f.svm);
        ok(
            &mut f.svm,
            policy_ix(&f.authority.pubkey(), policy_params(&assessor.pubkey())),
            &f.authority,
        );
        funded_hub(&mut f, 0, supply, 1);
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
            submit_ix(&f.operator.pubkey(), 0, evidence(2)),
            &f.operator,
        );
        ok(
            &mut f.svm,
            assess_ix(
                &assessor.pubkey(),
                &f.operator.pubkey(),
                0,
                assessment(1, 2, THRESHOLD),
            ),
            &assessor,
        );
        assert_eq!(
            milestone_of(&f.svm, 0).second_tranche_amount,
            supply - supply / 2
        );
        assert_eq!(
            balance(&f.svm, ata(&f.operator.pubkey(), &payment_mint())),
            supply
        );
        assert_eq!(balance(&f.svm, payment_vault(0)), 17);
        assert_eq!(balance(&f.svm, bond_vault(0)), BOND);
    }
}

#[test]
fn approval_recreates_an_operator_payment_account_closed_after_the_first_tranche() {
    let (mut f, assessor) = scenario();
    let key = ata(&f.operator.pubkey(), &payment_mint());
    let spend = anchor_spl::token::spl_token::instruction::transfer_checked(
        &TOKEN,
        &key,
        &payment_mint(),
        &ata(&f.buyer.pubkey(), &payment_mint()),
        &f.operator.pubkey(),
        &[],
        PRICE * SUPPLY / 2,
        6,
    )
    .unwrap();
    ok(&mut f.svm, spend, &f.operator);
    let close = anchor_spl::token::spl_token::instruction::close_account(
        &TOKEN,
        &key,
        &f.operator.pubkey(),
        &f.operator.pubkey(),
        &[],
    )
    .unwrap();
    ok(&mut f.svm, close, &f.operator);
    assert!(f.svm.get_account(&key).is_none());
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
            assessment(1, 2, THRESHOLD),
        ),
        &assessor,
    );
    assert_eq!(balance(&f.svm, key), PRICE * SUPPLY / 2);
    assert!(milestone_of(&f.svm, 0).second_tranche_released);
}
