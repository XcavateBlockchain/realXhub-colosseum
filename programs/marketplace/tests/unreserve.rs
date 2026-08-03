//! Unreserving, the one voluntary exit: an investor lets go of their
//! reservation while the sale is still filling. Once every share is reserved
//! the sale locks in and nobody backs out. Plus the crank that reclaims
//! cancelled positions once a listing stops selling.

mod common;
use common::*;

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

fn reserve(svm: &mut LiteSVM, investor: &Keypair, sponsor: &Keypair, amount: u32) {
    ok(
        svm,
        reserve_ix(&investor.pubkey(), &sponsor.pubkey(), 0, amount, u64::MAX),
        sponsor,
        &[sponsor, investor],
    );
}

#[test]
fn unreserve_releases_the_reservation() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    let before = tgbp_balance(&svm, &investor.pubkey());
    reserve(&mut svm, &investor, &sponsor, 10);

    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );

    // The money never moved, so nothing comes back; only the ledger lets go.
    assert_eq!(tgbp_balance(&svm, &investor.pubkey()), before);
    assert_eq!(
        reservation_of(&svm, &tgbp_acc(&investor.pubkey())).amount,
        0
    );
    assert_eq!(listing_of(&svm, 0).reserved_share_amount, 0);
    let position = position_of(&svm, 0, &investor.pubkey());
    assert!(position.cancelled);
    assert_eq!(position.reserved_share_amount, 0);
}

// The one-way bar: once out, this investor never buys this listing again.
#[test]
fn unreserve_blocks_rebuy() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    reserve(&mut svm, &investor, &sponsor, 10);
    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );

    fails_with(
        &mut svm,
        reserve_ix(&investor.pubkey(), &sponsor.pubkey(), 0, 10, u64::MAX),
        &sponsor,
        &[&sponsor, &investor],
        "PositionCancelled",
    );
}

#[test]
fn unreserve_twice_fails() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    reserve(&mut svm, &investor, &sponsor, 10);
    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );

    fails_with(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
        "NothingReserved",
    );
}

// The last reserved share locks the sale in: from there a reservation is
// claimed or expires, and the SPV can incorporate against a firm total.
#[test]
fn unreserve_stops_at_full_reservation() {
    let (mut svm, admin, sponsor) = setup_listed();
    let a = new_investor(&mut svm, &admin);
    let b = new_investor(&mut svm, &admin);
    let c = new_investor(&mut svm, &admin);
    let d = new_investor(&mut svm, &admin);
    reserve(&mut svm, &a, &sponsor, 34);
    reserve(&mut svm, &b, &sponsor, 33);
    // While the sale is still filling, backing out works.
    ok(&mut svm, unreserve_ix(&b.pubkey(), 0), &b, &[&b]);

    reserve(&mut svm, &c, &sponsor, 33);
    reserve(&mut svm, &d, &sponsor, 33);
    fails_with(
        &mut svm,
        unreserve_ix(&a.pubkey(), 0),
        &a,
        &[&a],
        "SaleLocked",
    );
}

// Sweeping missed reservations after the window drops the reserved count
// below full again, but the lock must hold: the deadline is the tell.
#[test]
fn unreserve_stays_barred_after_sweeps() {
    let (mut svm, admin, sponsor) = setup_listed();
    let a = new_investor(&mut svm, &admin);
    let b = new_investor(&mut svm, &admin);
    let c = new_investor(&mut svm, &admin);
    reserve(&mut svm, &a, &sponsor, 34);
    reserve(&mut svm, &b, &sponsor, 33);
    reserve(&mut svm, &c, &sponsor, 33);
    let confirmer = new_confirmer(&mut svm, &admin);
    ok(
        &mut svm,
        create_spv_ix(&confirmer.pubkey(), 0),
        &confirmer,
        &[&confirmer],
    );

    warp(&mut svm, CLAIMING_TIME + 1);
    let cranker = funded(&mut svm);
    ok(
        &mut svm,
        release_reservation_ix(&cranker.pubkey(), 0, &b.pubkey()),
        &cranker,
        &[&cranker],
    );
    fails_with(
        &mut svm,
        unreserve_ix(&a.pubkey(), 0),
        &a,
        &[&a],
        "SaleLocked",
    );
}

// Unreserving is an exit path: it works even after the investor's role and
// compliance are gone.
#[test]
fn unreserve_works_after_role_removed() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    reserve(&mut svm, &investor, &sponsor, 10);
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
    assert_eq!(listing_of(&svm, 0).reserved_share_amount, 0);
}

// ==================== the cancelled-position crank ====================

#[test]
fn close_cancelled_position_reclaims_rent() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    reserve(&mut svm, &investor, &sponsor, 10);
    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );
    let a = new_investor(&mut svm, &admin);
    let b = new_investor(&mut svm, &admin);
    let c = new_investor(&mut svm, &admin);
    acquire_many(&mut svm, &admin, &[(&a, 34), (&b, 33), (&c, 33)]);

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
    reserve(&mut svm, &investor, &sponsor, 10);
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
        reserve_ix(&investor.pubkey(), &sponsor.pubkey(), 0, 10, u64::MAX),
        &sponsor,
        &[&sponsor, &investor],
        "PositionCancelled",
    );
}

#[test]
fn close_requires_cancelled_position() {
    let (mut svm, admin, _sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    let b = new_investor(&mut svm, &admin);
    let c = new_investor(&mut svm, &admin);
    acquire_many(&mut svm, &admin, &[(&investor, 34), (&b, 33), (&c, 33)]);

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

// The crank must not depend on anyone flipping the status: once the expiry
// passes, cancelled positions are sweepable even if the listing never left
// Listed.
#[test]
fn close_works_after_expiry_without_status_flip() {
    let (mut svm, admin, sponsor) = setup_listed();
    let investor = new_investor(&mut svm, &admin);
    reserve(&mut svm, &investor, &sponsor, 10);
    ok(
        &mut svm,
        unreserve_ix(&investor.pubkey(), 0),
        &investor,
        &[&investor],
    );

    warp(&mut svm, LISTING_DURATION + 1);
    assert_eq!(listing_of(&svm, 0).status, ListingStatus::Listed);
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
}
