//! Property-based solvency: random walks over the whole primary lifecycle
//! (reserve, claim, buy, unreserve, expiry, both withdraws, the cranks,
//! teardown, and the SPV lawyer election) must keep the listing vault equal
//! to what paid positions are owed, every reservation equal to its
//! position's reserved cost, the share supply conserved between the
//! property vault and the holders, every
//! counter in agreement with the accounts, and every locked share backed by
//! exactly one live vote record. Cross-checking the accounts against each
//! other needs no model, so any drift in any writer shows up here.

mod common;
use common::*;

use anchor_spl::token_2022::spl_token_2022::{
    extension::StateWithExtensions, state::Account as TokenAccountState,
};
use proptest::prelude::*;

/// Upper bound on election rounds in a walk: one per op at most.
const MAX_ROUNDS: u64 = 16;

fn token_balance(svm: &LiteSVM, addr: &Pubkey) -> u64 {
    svm.get_account(addr)
        .filter(|a| !a.data.is_empty())
        .map(|a| {
            StateWithExtensions::<TokenAccountState>::unpack(&a.data)
                .unwrap()
                .base
                .amount
        })
        .unwrap_or(0)
}

fn account_alive(svm: &LiteSVM, addr: &Pubkey) -> bool {
    svm.get_account(addr).is_some_and(|a| !a.data.is_empty())
}

fn vote_record(
    svm: &LiteSVM,
    round: u64,
    voter: &Pubkey,
) -> Option<marketplace::state::LawyerVote> {
    let addr = lawyer_vote_pda(0, round, voter);
    svm.get_account(&addr)
        .filter(|a| !a.data.is_empty())
        .map(|a| marketplace::state::LawyerVote::try_deserialize(&mut &a.data[..]).unwrap())
}

fn candidacy(
    svm: &LiteSVM,
    round: u64,
    lawyer: &Pubkey,
) -> Option<marketplace::state::LawyerCandidacy> {
    let addr = candidacy_pda(0, round, lawyer);
    svm.get_account(&addr)
        .filter(|a| !a.data.is_empty())
        .map(|a| marketplace::state::LawyerCandidacy::try_deserialize(&mut &a.data[..]).unwrap())
}

fn check_invariants(
    svm: &LiteSVM,
    investors: &[Keypair],
    lawyer: &Pubkey,
) -> Result<(), TestCaseError> {
    if !account_alive(svm, &listing_pda(0)) {
        // Teardown ran; its guards promise nothing outlived the listing.
        for investor in investors {
            prop_assert!(!account_alive(svm, &position_pda(0, &investor.pubkey())));
            prop_assert!(!account_alive(svm, &holding_pda(0, &investor.pubkey())));
            for round in 1..=MAX_ROUNDS {
                prop_assert!(vote_record(svm, round, &investor.pubkey()).is_none());
            }
        }
        prop_assert!(!account_alive(svm, &property_pda(0)));
        prop_assert_eq!(token_balance(svm, &listing_payment_ata(0)), 0);
        prop_assert_eq!(token_balance(svm, &vault_share_account(0)), 0);
        return Ok(());
    }

    let mut owed = 0u64;
    let mut position_shares = 0u32;
    let mut reserved_shares = 0u32;
    let mut open_positions = 0u32;
    let mut holders = 0u32;
    let mut held_shares = 0u64;
    for investor in investors {
        let mut reserved_cost = 0u64;
        if account_alive(svm, &position_pda(0, &investor.pubkey())) {
            let p = position_of(svm, 0, &investor.pubkey());
            owed += p.paid_funds + p.paid_fee + p.paid_tax;
            reserved_cost = p.reserved_funds + p.reserved_fee + p.reserved_tax;
            position_shares += p.share_amount;
            reserved_shares += p.reserved_share_amount;
            open_positions += 1;
        }
        if account_alive(svm, &holding_pda(0, &investor.pubkey())) {
            holders += 1;
            held_shares += holding_of(svm, 0, &investor.pubkey()).amount as u64;
        }
        // The reservation record always matches the position's reserved cost.
        let reservation_acc = reservation_pda(&tgbp_acc(&investor.pubkey()));
        let reserved_amount = if account_alive(svm, &reservation_acc) {
            reservation_of(svm, &tgbp_acc(&investor.pubkey())).amount
        } else {
            0
        };
        prop_assert_eq!(reserved_amount, reserved_cost);
    }
    // Money: the vault holds exactly what open positions are owed.
    prop_assert_eq!(token_balance(svm, &listing_payment_ata(0)), owed);
    // Shares: conserved between the property vault and the holders.
    prop_assert_eq!(
        token_balance(svm, &vault_share_account(0)),
        SHARE_AMOUNT as u64 - held_shares
    );
    prop_assert_eq!(position_shares as u64, held_shares);
    // Counters: agree with the accounts they summarize.
    let listing = listing_of(svm, 0);
    prop_assert_eq!(listing.sold_share_amount, position_shares);
    prop_assert_eq!(listing.reserved_share_amount, reserved_shares);
    prop_assert_eq!(listing.position_count, open_positions);
    prop_assert_eq!(property_of(svm, 0).holder_count, holders);

    // Votes: every locked share is backed by exactly one live record, and
    // while a round runs, its tally equals the records behind it.
    let election = listing.spv_election;
    let mut current_round_power = 0u32;
    for investor in investors {
        let mut record_power = 0u32;
        for round in 1..=MAX_ROUNDS {
            if let Some(record) = vote_record(svm, round, &investor.pubkey()) {
                record_power += record.power;
                if round == election.round && election.expiry != 0 {
                    current_round_power += record.power;
                }
            }
        }
        if account_alive(svm, &holding_pda(0, &investor.pubkey())) {
            prop_assert_eq!(
                holding_of(svm, 0, &investor.pubkey()).locked_amount,
                record_power
            );
        } else {
            prop_assert_eq!(record_power, 0);
        }
    }
    if election.expiry != 0 {
        let tally = candidacy(svm, election.round, lawyer)
            .map(|c| c.vote_power)
            .unwrap_or(0);
        prop_assert_eq!(tally, current_round_power);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]
    #[test]
    fn random_lifecycle_walks_stay_solvent(
        ops in proptest::collection::vec((0u8..3u8, 0u8..16u8, 1u32..30u32), 1..14)
    ) {
        let (mut svm, admin, _authority) = setup();
        let operator = funded(&mut svm);
        seed_region(&mut svm, 1, &operator.pubkey());
        seed_location(&mut svm, 1, POSTCODE);
        let developer = new_developer(&mut svm, &admin);
        ok(&mut svm, list_ix(&developer.pubkey(), 0), &developer, &[&developer]);
        ok(&mut svm, init_assets_ix(&developer.pubkey(), 0), &developer, &[&developer]);
        let sponsor = sponsor();
        let investors = [
            new_investor(&mut svm, &admin),
            new_investor(&mut svm, &admin),
            new_investor(&mut svm, &admin),
        ];
        let lawyer = new_registered_lawyer(&mut svm, &admin, 1);
        let confirmer = new_confirmer(&mut svm, &admin);

        for (who, kind, amount) in ops {
            let investor = &investors[who as usize];
            // The election ops read the listing to aim their round argument,
            // which a torn-down walk no longer has.
            if (7..=10).contains(&kind) && !account_alive(&svm, &listing_pda(0)) {
                continue;
            }
            // Failures (cap, cancelled, expired, wrong state, wrong round)
            // are part of the exercise; the invariants must hold either way.
            match kind {
                0 => {
                    let ix = buy_ix(&investor.pubkey(), &sponsor.pubkey(), 0, amount, u64::MAX);
                    let _ = process(&mut svm, ix, &sponsor, &[&sponsor, investor]);
                }
                1 => {
                    let ix = unreserve_ix(&investor.pubkey(), 0);
                    let _ = process(&mut svm, ix, investor, &[investor]);
                }
                2 => warp(&mut svm, LISTING_DURATION + 1),
                3 => {
                    let ix = withdraw_expired_ix(&investor.pubkey(), 0);
                    let _ = process(&mut svm, ix, investor, &[investor]);
                }
                4 => {
                    let ix = close_position_ix(&investor.pubkey(), 0, &investors[0].pubkey());
                    let _ = process(&mut svm, ix, investor, &[investor]);
                }
                5 => {
                    let ix = withdraw_deposit_ix(&developer.pubkey(), 0);
                    let _ = process(&mut svm, ix, &developer, &[&developer]);
                }
                6 => {
                    let ix = create_spv_ix(&confirmer.pubkey(), 0);
                    let _ = process(&mut svm, ix, &confirmer, &[&confirmer]);
                }
                7 => {
                    let election = listing_of(&svm, 0).spv_election;
                    let round = if election.expiry == 0 {
                        election.round + 1
                    } else {
                        election.round
                    };
                    let ix = claim_spv_ix(&lawyer.pubkey(), 0, round, 1_000_000_000);
                    let _ = process(&mut svm, ix, &sponsor, &[&sponsor, &lawyer]);
                }
                8 => {
                    // One lawyer stands in these walks, so every vote and
                    // revote targets the same candidacy and never needs the
                    // previous one.
                    let round = listing_of(&svm, 0).spv_election.round;
                    let ix = vote_spv_ix(
                        &investor.pubkey(),
                        0,
                        round,
                        &lawyer.pubkey(),
                        None,
                        amount,
                    );
                    let _ = process(&mut svm, ix, &sponsor, &[&sponsor, investor]);
                }
                9 => {
                    let round = listing_of(&svm, 0).spv_election.round;
                    let mut candidates = Vec::new();
                    if candidacy(&svm, round, &lawyer.pubkey()).is_some() {
                        candidates.push(lawyer.pubkey());
                    }
                    let ix = finalize_spv_ix(
                        &investor.pubkey(),
                        0,
                        round,
                        Some(&lawyer.pubkey()),
                        &candidates,
                    );
                    let _ = process(&mut svm, ix, investor, &[investor]);
                }
                10 => {
                    let round = listing_of(&svm, 0).spv_election.round;
                    let ix = unlock_votes_ix(&investor.pubkey(), 0, round);
                    let _ = process(&mut svm, ix, investor, &[investor]);
                }
                11 => {
                    let ix =
                        reserve_ix(&investor.pubkey(), &sponsor.pubkey(), 0, amount, u64::MAX);
                    let _ = process(&mut svm, ix, &sponsor, &[&sponsor, investor]);
                }
                12 => {
                    let ix = claim_ix(&investor.pubkey(), &sponsor.pubkey(), 0);
                    let _ = process(&mut svm, ix, &sponsor, &[&sponsor, investor]);
                }
                13 => {
                    // Reserve whatever is left, aiming for the lock-in that
                    // create_spv and the claims need.
                    let listing = listing_of(&svm, 0);
                    let left = listing.listed_share_amount
                        - listing.sold_share_amount
                        - listing.reserved_share_amount;
                    let ix =
                        reserve_ix(&investor.pubkey(), &sponsor.pubkey(), 0, left, u64::MAX);
                    let _ = process(&mut svm, ix, &sponsor, &[&sponsor, investor]);
                }
                14 => {
                    let ix =
                        release_reservation_ix(&investor.pubkey(), 0, &investors[0].pubkey());
                    let _ = process(&mut svm, ix, investor, &[investor]);
                }
                _ => {
                    let ix = close_dead_listing_ix(
                        &investor.pubkey(),
                        0,
                        &developer.pubkey(),
                        true,
                    );
                    let _ = process(&mut svm, ix, investor, &[investor]);
                }
            }
            check_invariants(&svm, &investors, &lawyer.pubkey())?;
        }
    }
}
