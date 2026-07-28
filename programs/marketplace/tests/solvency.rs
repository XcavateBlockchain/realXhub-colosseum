//! Property-based solvency: random walks over the whole primary lifecycle
//! (buy, unreserve, expiry, both withdraws, the cranks, teardown) must keep
//! the listing vault equal to what open positions are owed, the share supply
//! conserved between the property vault and the holders, and every counter in
//! agreement with the accounts. Cross-checking the accounts against each
//! other needs no model, so any drift in any writer shows up here.

mod common;
use common::*;

use anchor_spl::token_2022::spl_token_2022::{
    extension::StateWithExtensions, state::Account as TokenAccountState,
};
use proptest::prelude::*;

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

fn check_invariants(svm: &LiteSVM, investors: &[Keypair]) -> Result<(), TestCaseError> {
    if !account_alive(svm, &listing_pda(0)) {
        // Teardown ran; its guards promise nothing outlived the listing.
        for investor in investors {
            prop_assert!(!account_alive(svm, &position_pda(0, &investor.pubkey())));
            prop_assert!(!account_alive(svm, &holding_pda(0, &investor.pubkey())));
        }
        prop_assert!(!account_alive(svm, &property_pda(0)));
        prop_assert_eq!(token_balance(svm, &listing_payment_ata(0)), 0);
        prop_assert_eq!(token_balance(svm, &vault_share_account(0)), 0);
        return Ok(());
    }

    let mut owed = 0u64;
    let mut position_shares = 0u32;
    let mut open_positions = 0u32;
    let mut holders = 0u32;
    let mut held_shares = 0u64;
    for investor in investors {
        if account_alive(svm, &position_pda(0, &investor.pubkey())) {
            let p = position_of(svm, 0, &investor.pubkey());
            owed += p.paid_funds + p.paid_fee + p.paid_tax;
            position_shares += p.share_amount;
            open_positions += 1;
        }
        if account_alive(svm, &holding_pda(0, &investor.pubkey())) {
            holders += 1;
            held_shares += holding_of(svm, 0, &investor.pubkey()).amount as u64;
        }
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
    prop_assert_eq!(listing.position_count, open_positions);
    prop_assert_eq!(property_of(svm, 0).holder_count, holders);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]
    #[test]
    fn random_lifecycle_walks_stay_solvent(
        ops in proptest::collection::vec((0u8..3u8, 0u8..7u8, 1u32..30u32), 1..14)
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

        for (who, kind, amount) in ops {
            let investor = &investors[who as usize];
            // Failures (cap, cancelled, expired, wrong state) are part of the
            // exercise; the invariants must hold either way.
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
                _ => {
                    let ix = close_dead_listing_ix(
                        &investor.pubkey(),
                        0,
                        &developer.pubkey(),
                        true,
                        &[listing_payment_ata(0)],
                    );
                    let _ = process(&mut svm, ix, investor, &[investor]);
                }
            }
            check_invariants(&svm, &investors)?;
        }
    }
}
