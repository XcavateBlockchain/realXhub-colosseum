//! Unreserving: the full refund, the one-way re-buy bar, and the crank that
//! reclaims cancelled positions once a listing stops selling.

mod common;
use common::*;

use anchor_spl::token_2022::spl_token_2022::{
    extension::StateWithExtensions,
    state::{Account as TokenAccountState, AccountState},
};
use marketplace::state::ListingStatus;

/// Full pipeline up to an open listing, plus the sponsor.
fn setup_listed() -> (LiteSVM, Keypair, Keypair) {
    let (mut svm, admin, _authority) = setup();
    let operator = funded(&mut svm);
    seed_region(&mut svm, 1, &operator.pubkey());
    seed_location(&mut svm, 1, POSTCODE);
    let developer = new_developer(&mut svm, &admin);
    ok(
        &mut svm,
        list_ix(&developer.pubkey(), 0),
        &developer,
        &[&developer],
    );
    ok(
        &mut svm,
        init_assets_ix(&developer.pubkey(), 0),
        &developer,
        &[&developer],
    );
    (svm, admin, sponsor())
}

fn buy(svm: &mut LiteSVM, investor: &Keypair, sponsor: &Keypair, amount: u32) {
    ok(
        svm,
        buy_ix(&investor.pubkey(), &sponsor.pubkey(), 0, amount, u64::MAX),
        sponsor,
        &[sponsor, investor],
    );
}

fn sell_out(svm: &mut LiteSVM, admin: &Keypair, sponsor: &Keypair) {
    let a = new_investor(svm, admin);
    let b = new_investor(svm, admin);
    let c = new_investor(svm, admin);
    buy(svm, &a, sponsor, 34);
    buy(svm, &b, sponsor, 33);
    buy(svm, &c, sponsor, 33);
}

#[test]
fn unreserve_refunds_everything_and_returns_shares() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    let tgbp_before = tgbp_balance(&svm, &investor.pubkey());
    buy(&mut svm, &investor, &sponsor, 10);
    buy(&mut svm, &investor, &sponsor, 15);

    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );

    // Money: everything back, fee and tax included.
    assert_eq!(tgbp_balance(&svm, &investor.pubkey()), tgbp_before);
    let vault_acc = svm.get_account(&listing_payment_ata(0)).unwrap();
    let vault_state = StateWithExtensions::<TokenAccountState>::unpack(&vault_acc.data).unwrap();
    assert_eq!(vault_state.base.amount, 0);

    // Shares: back in the property vault; the investor's account is empty and
    // locked again.
    let vault_shares = svm.get_account(&vault_share_account(0)).unwrap();
    let vault_shares =
        StateWithExtensions::<TokenAccountState>::unpack(&vault_shares.data).unwrap();
    assert_eq!(vault_shares.base.amount, SHARE_AMOUNT as u64);
    let share_acc = svm
        .get_account(&investor_share_ata(0, &investor.pubkey()))
        .unwrap();
    let share_state = StateWithExtensions::<TokenAccountState>::unpack(&share_acc.data).unwrap();
    assert_eq!(share_state.base.amount, 0);
    assert_eq!(share_state.base.state, AccountState::Frozen);

    // Ledger: listing reopened in full, holding closed, position flagged.
    assert_eq!(listing_of(&svm, 0).sold_share_amount, 0);
    assert_eq!(property_of(&svm, 0).holder_count, 0);
    assert!(svm
        .get_account(&holding_pda(0, &investor.pubkey()))
        .is_none_or(|a| a.data.is_empty()));
    let position = position_of(&svm, 0, &investor.pubkey());
    assert!(position.cancelled);
    assert_eq!(position.share_amount, 0);
    assert_eq!(position.paid_funds, 0);
}

// The one-way bar: once cancelled, this investor never buys this listing
// again.
#[test]
fn unreserve_blocks_rebuy() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    buy(&mut svm, &investor, &sponsor, 10);
    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );

    fails_with(
        &mut svm,
        buy_ix(&investor.pubkey(), &sponsor.pubkey(), 0, 10, u64::MAX),
        &sponsor,
        &[&sponsor, &investor],
        "PositionCancelled",
    );
}

#[test]
fn unreserve_twice_fails() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    buy(&mut svm, &investor, &sponsor, 10);
    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );

    // The holding is gone, so the second attempt can't even resolve it.
    fails_with(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
        "AccountNotInitialized",
    );
}

#[test]
fn unreserve_after_sellout_fails() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    buy(&mut svm, &investor, &sponsor, 34);
    let b = new_investor(&mut svm, &admin);
    let c = new_investor(&mut svm, &admin);
    buy(&mut svm, &b, &sponsor, 33);
    buy(&mut svm, &c, &sponsor, 33);
    assert_eq!(listing_of(&svm, 0).status, ListingStatus::SoldOut);

    // The sale is locked in; the legal phase owns the funds now.
    fails_with(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
        "ListingNotActive",
    );
}

// Unreserving is an exit path: it works even after the investor's role and
// compliance are gone.
#[test]
fn unreserve_works_after_role_removed() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    let tgbp_before = tgbp_balance(&svm, &investor.pubkey());
    buy(&mut svm, &investor, &sponsor, 10);
    ok(
        &mut svm,
        roles_remove_ix(
            &admin.pubkey(),
            &investor.pubkey(),
            Role::RealEstateInvestor,
            &admin.pubkey(),
        ),
        &admin,
        &[&admin],
    );

    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );
    assert_eq!(tgbp_balance(&svm, &investor.pubkey()), tgbp_before);
}

// A position paid in the 6-decimal mint refunds in that mint, at its scale.
#[test]
fn unreserve_refunds_in_position_mint() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    give_gbp6(&mut svm, &investor.pubkey(), 1_000_000_000);

    let vault_ata = Pubkey::find_program_address(
        &[
            listing_vault_pda(0).as_ref(),
            anchor_spl::token::ID.as_ref(),
            gbp6_mint().as_ref(),
        ],
        &anchor_spl::associated_token::ID,
    )
    .0;
    ok(
        &mut svm,
        buy_ix_with_mint(
            &investor.pubkey(),
            &sponsor.pubkey(),
            0,
            10,
            u64::MAX,
            gbp6_mint(),
            gbp6_acc(&investor.pubkey()),
            vault_ata,
        ),
        &sponsor,
        &[&sponsor, &investor],
    );

    ok(
        &mut svm,
        unreserve_ix_with_mint(
            &investor.pubkey(),
            0,
            gbp6_mint(),
            gbp6_acc(&investor.pubkey()),
            vault_ata,
            anchor_spl::token::ID,
        ),
        &investor,
        &[&investor],
    );

    let acc = svm.get_account(&gbp6_acc(&investor.pubkey())).unwrap();
    let state = StateWithExtensions::<TokenAccountState>::unpack(&acc.data).unwrap();
    assert_eq!(state.base.amount, 1_000_000_000);
}

#[test]
fn close_cancelled_position_reclaims_rent() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    buy(&mut svm, &investor, &sponsor, 10);
    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );
    sell_out(&mut svm, &admin, &sponsor);

    let sponsor_before = svm.get_account(&sponsor.pubkey()).unwrap().lamports;
    let cranker = funded(&mut svm);
    ok(
        &mut svm,
        close_position_ix(&cranker.pubkey(), 0, &investor.pubkey()),
        &cranker,
        &[&cranker],
    );

    assert!(svm
        .get_account(&position_pda(0, &investor.pubkey()))
        .is_none_or(|a| a.data.is_empty()));
    assert!(svm.get_account(&sponsor.pubkey()).unwrap().lamports > sponsor_before);
}

// The bar must outlive the crank: while the listing sells, a cancelled
// position stays open, so cancel-close-rebuy can't work.
#[test]
fn close_while_listing_active_fails() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    buy(&mut svm, &investor, &sponsor, 10);
    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );

    let cranker = funded(&mut svm);
    fails_with(
        &mut svm,
        close_position_ix(&cranker.pubkey(), 0, &investor.pubkey()),
        &cranker,
        &[&cranker],
        "ListingStillActive",
    );
    // And the re-buy stays barred the whole time.
    fails_with(
        &mut svm,
        buy_ix(&investor.pubkey(), &sponsor.pubkey(), 0, 10, u64::MAX),
        &sponsor,
        &[&sponsor, &investor],
        "PositionCancelled",
    );
}

#[test]
fn close_requires_cancelled_position() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    buy(&mut svm, &investor, &sponsor, 34);
    let b = new_investor(&mut svm, &admin);
    let c = new_investor(&mut svm, &admin);
    buy(&mut svm, &b, &sponsor, 33);
    buy(&mut svm, &c, &sponsor, 33);

    // A live position holds real settlement accounting; the crank can't touch it.
    let cranker = funded(&mut svm);
    fails_with(
        &mut svm,
        close_position_ix(&cranker.pubkey(), 0, &investor.pubkey()),
        &cranker,
        &[&cranker],
        "PositionNotCancelled",
    );
}
