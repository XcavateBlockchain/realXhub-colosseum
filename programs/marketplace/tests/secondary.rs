//! The secondary share market: relist part of a holding, delist it again,
//! and buy relisted shares with both parties' income settled at their
//! pre-trade balances. Trades exist only on finalized properties; listed
//! shares stay on the seller's ledger but leave their voting and transfer
//! headroom.

mod common;
use common::*;

use anchor_lang::{AnchorSerialize, Discriminator};
use marketplace::state::ListingStatus;
use solana_account::Account;

const COSTS: u64 = 1_000_000_000;
const DOCS: [u8; 32] = [7u8; 32];
const ASK: u64 = 6_000_000_000;

/// (shares, pays in gbp6) per investor; a mixed-mint sellout of 100.
const BUYS: [(u32, bool); 4] = [(34, false), (33, false), (24, true), (9, true)];

/// A settled, finalized property. Investors hold 34/33/24/9; the first one
/// still carries their 34-share lawyer-election lock.
fn finalized_property() -> (LiteSVM, Keypair, Vec<Keypair>) {
    build_property(true)
}

/// Same flow; `finalize` false stops at `Legal`, holders already claimed.
fn build_property(finalize: bool) -> (LiteSVM, Keypair, Vec<Keypair>) {
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

    let spn = sponsor();
    let investors: Vec<Keypair> = (0..4).map(|_| new_investor(&mut svm, &admin)).collect();
    for (investor, (shares, gbp6)) in investors.iter().zip(BUYS) {
        let ix = if gbp6 {
            give_gbp6(&mut svm, &investor.pubkey(), 1_000_000_000);
            reserve_ix_with_mint(
                &investor.pubkey(),
                &spn.pubkey(),
                0,
                shares,
                u64::MAX,
                gbp6_mint(),
                gbp6_acc(&investor.pubkey()),
            )
        } else {
            reserve_ix(&investor.pubkey(), &spn.pubkey(), 0, shares, u64::MAX)
        };
        ok(&mut svm, ix, &spn, &[&spn, investor]);
    }
    let confirmer = new_confirmer(&mut svm, &admin);
    ok(
        &mut svm,
        create_spv_ix(&confirmer.pubkey(), 0),
        &confirmer,
        &[&confirmer],
    );
    for (investor, (_, gbp6)) in investors.iter().zip(BUYS) {
        let ix = if gbp6 {
            claim_ix_with_mint(
                &investor.pubkey(),
                &spn.pubkey(),
                0,
                gbp6_mint(),
                gbp6_acc(&investor.pubkey()),
                payment_ata(&listing_vault_pda(0), &gbp6_mint()),
            )
        } else {
            claim_ix(&investor.pubkey(), &spn.pubkey(), 0)
        };
        ok(&mut svm, ix, &spn, &[&spn, investor]);
    }

    let dl = new_registered_lawyer(&mut svm, &admin, 1);
    ok(
        &mut svm,
        assign_dev_lawyer_ix(&developer.pubkey(), 0, &dl.pubkey()),
        &developer,
        &[&developer],
    );
    let sl = new_registered_lawyer(&mut svm, &admin, 1);
    ok(
        &mut svm,
        claim_spv_ix(&sl.pubkey(), 0, 1, COSTS),
        &sl,
        &[&sl, &spn],
    );
    ok(
        &mut svm,
        vote_spv_ix(&investors[0].pubkey(), 0, 1, &sl.pubkey(), None, 34),
        &spn,
        &[&spn, &investors[0]],
    );
    warp(&mut svm, 10_001);
    ok(
        &mut svm,
        finalize_spv_ix(&operator.pubkey(), 0, 1, Some(&sl.pubkey()), &[sl.pubkey()]),
        &operator,
        &[&operator],
    );
    for lawyer in [&dl, &sl] {
        ok(
            &mut svm,
            confirm_docs_ix(&lawyer.pubkey(), 0, true, DOCS),
            lawyer,
            &[lawyer],
        );
    }

    for wallet in [
        &developer.pubkey(),
        &dl.pubkey(),
        &sl.pubkey(),
        &operator.pubkey(),
    ] {
        give_tgbp(&mut svm, wallet, 0);
        give_gbp6(&mut svm, wallet, 0);
    }
    set_token_account_for(
        &mut svm,
        tgbp_mint(),
        treasury_payment_ata(),
        &treasury(),
        0,
    );
    set_token_account_for(
        &mut svm,
        gbp6_mint(),
        payment_ata(&treasury(), &gbp6_mint()),
        &treasury(),
        0,
    );
    if finalize {
        let cranker = funded(&mut svm);
        ok(
            &mut svm,
            execute_deal_ix(
                &cranker.pubkey(),
                0,
                1,
                &developer.pubkey(),
                &dl.pubkey(),
                &sl.pubkey(),
                &operator.pubkey(),
                &[tgbp_mint(), gbp6_mint()],
            ),
            &cranker,
            &[&cranker],
        );
        assert_eq!(listing_of(&svm, 0).status, ListingStatus::Finalized);
    }
    (svm, admin, investors)
}

fn relist(svm: &mut LiteSVM, seller: &Keypair, id: u64, amount: u32) {
    ok(
        svm,
        relist_ix(&seller.pubkey(), 0, id, amount, ASK),
        seller,
        &[seller],
    );
}

/// Write the property program's income ledger with one tGBP stream that has
/// accrued `per_share` per share, so settlements have something to bank.
fn seed_income_stream(svm: &mut LiteSVM, per_share: u128) {
    let income = property::state::PropertyIncome {
        asset_id: 0,
        streams: vec![property::state::IncomeStream {
            mint: tgbp_mint(),
            per_share,
            dust: 0,
        }],
        rent_payer: Pubkey::new_unique(),
        bump: Pubkey::find_program_address(
            &[b"income", &0u64.to_le_bytes()],
            &marketplace::PROPERTY_PROGRAM,
        )
        .1,
    };
    let mut data = property::state::PropertyIncome::DISCRIMINATOR.to_vec();
    income.serialize(&mut data).unwrap();
    svm.set_account(
        property_income_pda(0),
        Account {
            lamports: 100_000_000,
            data,
            owner: marketplace::PROPERTY_PROGRAM,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn checkpoint_of(svm: &LiteSVM, owner: &Pubkey) -> property::state::IncomeCheckpoint {
    let acc = svm.get_account(&property_checkpoint_pda(0, owner)).unwrap();
    property::state::IncomeCheckpoint::try_deserialize(&mut acc.data.as_slice()).unwrap()
}

#[test]
fn settle_income_discriminator_matches_the_property_program() {
    assert_eq!(
        marketplace::instructions::secondary::SETTLE_INCOME_DISC,
        <property::instruction::SettleIncome as Discriminator>::DISCRIMINATOR
    );
}

// --- relist / delist ---

#[test]
fn relist_reserves_the_shares() {
    let (mut svm, _admin, investors) = finalized_property();
    let seller = &investors[1];
    relist(&mut svm, seller, 0, 20);

    let listing = share_listing_of(&svm, 0);
    assert_eq!(listing.seller, seller.pubkey());
    assert_eq!(listing.asset_id, 0);
    assert_eq!(listing.amount, 20);
    assert_eq!(listing.share_price, ASK);
    assert_eq!(listing.fee_bps, 100);

    let holding = holding_of(&svm, 0, &seller.pubkey());
    assert_eq!(holding.amount, 33);
    assert_eq!(holding.listed, 20);
    assert_eq!(holding.transferable(), 13);

    // A second listing works against the reduced headroom, ids are
    // monotonic, and overshooting what's left is rejected.
    relist(&mut svm, seller, 1, 13);
    assert_eq!(holding_of(&svm, 0, &seller.pubkey()).listed, 33);
    fails_with(
        &mut svm,
        relist_ix(&seller.pubkey(), 0, 2, 1, ASK),
        seller,
        &[seller],
        "NotEnoughShares",
    );
}

#[test]
fn relist_rejects_vote_locked_shares() {
    let (mut svm, _admin, investors) = finalized_property();
    // Investor 0 still carries their 34-share election lock.
    let seller = &investors[0];
    fails_with(
        &mut svm,
        relist_ix(&seller.pubkey(), 0, 0, 1, ASK),
        seller,
        &[seller],
        "NotEnoughShares",
    );
    ok(
        &mut svm,
        unlock_votes_ix(&seller.pubkey(), 0, 1),
        seller,
        &[seller],
    );
    relist(&mut svm, seller, 0, 34);
}

#[test]
fn relist_waits_for_the_settlement() {
    let (mut svm, _admin, investors) = build_property(false);
    assert_eq!(listing_of(&svm, 0).status, ListingStatus::Legal);
    let seller = &investors[1];
    fails_with(
        &mut svm,
        relist_ix(&seller.pubkey(), 0, 0, 10, ASK),
        seller,
        &[seller],
        "PropertyNotFinalized",
    );
}

#[test]
fn relist_validates_the_ask() {
    let (mut svm, _admin, investors) = finalized_property();
    let seller = &investors[1];
    fails_with(
        &mut svm,
        relist_ix(&seller.pubkey(), 0, 0, 0, ASK),
        seller,
        &[seller],
        "InvalidShareAmount",
    );
    // Below one whole unit of the smallest-decimals mint.
    fails_with(
        &mut svm,
        relist_ix(&seller.pubkey(), 0, 0, 10, 999),
        seller,
        &[seller],
        "InvalidSharePrice",
    );
}

#[test]
fn delist_frees_the_reserve() {
    let (mut svm, _admin, investors) = finalized_property();
    let seller = &investors[1];
    relist(&mut svm, seller, 0, 20);

    // Another holder can't delist someone else's listing, even naming the
    // right rent payer.
    let outsider = &investors[2];
    let mut hijack = delist_ix(&outsider.pubkey(), 0, 0);
    hijack.accounts[1].pubkey = seller.pubkey();
    fails_with(&mut svm, hijack, outsider, &[outsider], "WrongSeller");
    let before = svm.get_balance(&seller.pubkey()).unwrap();
    ok(
        &mut svm,
        delist_ix(&seller.pubkey(), 0, 0),
        seller,
        &[seller],
    );
    assert_eq!(holding_of(&svm, 0, &seller.pubkey()).listed, 0);
    assert!(svm.get_account(&share_listing_pda(0)).is_none());
    assert!(svm.get_balance(&seller.pubkey()).unwrap() > before);
}

// --- buying ---

#[test]
fn buy_pays_seller_and_treasury_and_moves_shares() {
    let (mut svm, admin, investors) = finalized_property();
    let seller = &investors[1];
    relist(&mut svm, seller, 0, 20);

    let buyer = new_investor(&mut svm, &admin);
    give_tgbp(&mut svm, &buyer.pubkey(), 150_000_000_000);
    let treasury_before = token_balance(&svm, &treasury_payment_ata());
    ok(
        &mut svm,
        buy_relisted_ix(&buyer.pubkey(), 0, 0, &seller.pubkey(), 12, u64::MAX),
        &buyer,
        &[&buyer],
    );

    // 12 shares at 6 GBP = 72 GBP; 1% fee to the treasury, rest to the
    // seller's ATA.
    assert_eq!(tgbp_balance(&svm, &buyer.pubkey()), 78_000_000_000);
    assert_eq!(
        token_balance(&svm, &payment_ata(&seller.pubkey(), &tgbp_mint())),
        71_280_000_000
    );
    assert_eq!(
        token_balance(&svm, &treasury_payment_ata()) - treasury_before,
        720_000_000
    );

    // Ledger and token side agree; the listing keeps the remainder.
    let seller_holding = holding_of(&svm, 0, &seller.pubkey());
    assert_eq!(seller_holding.amount, 21);
    assert_eq!(seller_holding.listed, 8);
    let buyer_holding = holding_of(&svm, 0, &buyer.pubkey());
    assert_eq!(buyer_holding.amount, 12);
    assert_eq!(
        token_balance(&svm, &investor_share_ata(0, &seller.pubkey())),
        21
    );
    assert_eq!(
        token_balance(&svm, &investor_share_ata(0, &buyer.pubkey())),
        12
    );
    assert_eq!(property_of(&svm, 0).holder_count, 5);
    assert_eq!(share_listing_of(&svm, 0).amount, 8);

    // The rest sells out; the listing closes.
    ok(
        &mut svm,
        buy_relisted_ix(&buyer.pubkey(), 0, 0, &seller.pubkey(), 8, u64::MAX),
        &buyer,
        &[&buyer],
    );
    assert!(svm.get_account(&share_listing_pda(0)).is_none());
    assert_eq!(holding_of(&svm, 0, &seller.pubkey()).listed, 0);
    assert_eq!(holding_of(&svm, 0, &buyer.pubkey()).amount, 20);
}

#[test]
fn buy_validates_the_request() {
    let (mut svm, admin, investors) = finalized_property();
    let seller = &investors[1];
    relist(&mut svm, seller, 0, 10);
    let buyer = new_investor(&mut svm, &admin);
    give_tgbp(&mut svm, &buyer.pubkey(), 100_000_000_000);

    fails_with(
        &mut svm,
        buy_relisted_ix(&buyer.pubkey(), 0, 0, &seller.pubkey(), 11, u64::MAX),
        &buyer,
        &[&buyer],
        "NotEnoughSharesListed",
    );
    // 10 shares cost 60 GBP; a cap below that must reject.
    fails_with(
        &mut svm,
        buy_relisted_ix(&buyer.pubkey(), 0, 0, &seller.pubkey(), 10, 59_999_999_999),
        &buyer,
        &[&buyer],
        "CostTooHigh",
    );
    give_xcav(&mut svm, &buyer.pubkey(), 1_000_000_000);
    fails_with(
        &mut svm,
        buy_relisted_ix_with_mint(
            &buyer.pubkey(),
            0,
            0,
            &seller.pubkey(),
            10,
            u64::MAX,
            xcav_mint(),
            token_acc(&buyer.pubkey()),
        ),
        &buyer,
        &[&buyer],
        "MintNotAccepted",
    );
}

#[test]
fn buy_respects_the_ownership_cap() {
    let (mut svm, _admin, investors) = finalized_property();
    // Cap is 50% of 100 shares, strictly below: 49 at most. Investor 0
    // holds 34 and may only add 15 more.
    let seller = &investors[1];
    relist(&mut svm, seller, 0, 20);
    let buyer = &investors[0];
    give_tgbp(&mut svm, &buyer.pubkey(), 200_000_000_000);
    ok(
        &mut svm,
        unlock_votes_ix(&buyer.pubkey(), 0, 1),
        buyer,
        &[buyer],
    );
    fails_with(
        &mut svm,
        buy_relisted_ix(&buyer.pubkey(), 0, 0, &seller.pubkey(), 16, u64::MAX),
        buyer,
        &[buyer],
        "MaxOwnershipExceeded",
    );
    ok(
        &mut svm,
        buy_relisted_ix(&buyer.pubkey(), 0, 0, &seller.pubkey(), 15, u64::MAX),
        buyer,
        &[buyer],
    );
    assert_eq!(holding_of(&svm, 0, &buyer.pubkey()).amount, 49);
}

#[test]
fn buying_own_listing_is_blocked() {
    let (mut svm, _admin, investors) = finalized_property();
    let seller = &investors[1];
    relist(&mut svm, seller, 0, 10);
    give_tgbp(&mut svm, &seller.pubkey(), 100_000_000_000);
    fails_with(
        &mut svm,
        buy_relisted_ix(&seller.pubkey(), 0, 0, &seller.pubkey(), 5, u64::MAX),
        seller,
        &[seller],
        "DuplicateMutableAccount",
    );
}

#[test]
fn buy_settles_income_for_both_sides() {
    let (mut svm, admin, investors) = finalized_property();
    svm.add_program(marketplace::PROPERTY_PROGRAM, &program_bytes("property"))
        .unwrap();
    // 2 GBP per share accrued before the trade.
    seed_income_stream(&mut svm, 2_000_000_000);

    let seller = &investors[1];
    relist(&mut svm, seller, 0, 20);
    let buyer = new_investor(&mut svm, &admin);
    give_tgbp(&mut svm, &buyer.pubkey(), 100_000_000_000);
    ok(
        &mut svm,
        buy_relisted_ix(&buyer.pubkey(), 0, 0, &seller.pubkey(), 12, u64::MAX),
        &buyer,
        &[&buyer],
    );

    // The seller banked 33 shares' worth at the pre-trade balance; the
    // buyer's checkpoint opened at the current accumulator with nothing
    // banked, so they earn only from here on.
    let seller_cp = checkpoint_of(&svm, &seller.pubkey());
    assert_eq!(seller_cp.entries[0].pending, 66_000_000_000);
    assert_eq!(seller_cp.entries[0].per_share, 2_000_000_000);
    let buyer_cp = checkpoint_of(&svm, &buyer.pubkey());
    assert_eq!(buyer_cp.entries[0].pending, 0);
    assert_eq!(buyer_cp.entries[0].per_share, 2_000_000_000);
}

#[test]
fn buy_needs_the_real_income_ledger() {
    let (mut svm, admin, investors) = finalized_property();
    seed_income_stream(&mut svm, 2_000_000_000);
    let seller = &investors[1];
    relist(&mut svm, seller, 0, 10);
    let buyer = new_investor(&mut svm, &admin);
    give_tgbp(&mut svm, &buyer.pubkey(), 100_000_000_000);

    // A stand-in income account can't skip the settlements.
    let mut ix = buy_relisted_ix(&buyer.pubkey(), 0, 0, &seller.pubkey(), 5, u64::MAX);
    let fake = Pubkey::new_unique();
    for meta in ix.accounts.iter_mut() {
        if meta.pubkey == property_income_pda(0) {
            meta.pubkey = fake;
        }
    }
    fails_with(&mut svm, ix, &buyer, &[&buyer], "WrongVaultAccount");
}

// --- emptied holdings ---

#[test]
fn emptied_holding_closes_and_leaves_the_count() {
    let (mut svm, admin, investors) = finalized_property();
    let seller = &investors[3];
    relist(&mut svm, seller, 0, 9);
    let buyer = new_investor(&mut svm, &admin);
    give_tgbp(&mut svm, &buyer.pubkey(), 100_000_000_000);
    ok(
        &mut svm,
        buy_relisted_ix(&buyer.pubkey(), 0, 0, &seller.pubkey(), 9, u64::MAX),
        &buyer,
        &[&buyer],
    );
    assert_eq!(holding_of(&svm, 0, &seller.pubkey()).amount, 0);
    assert_eq!(property_of(&svm, 0).holder_count, 5);

    let cranker = funded(&mut svm);
    ok(
        &mut svm,
        close_holding_ix(&cranker.pubkey(), 0, &seller.pubkey()),
        &cranker,
        &[&cranker],
    );
    assert!(svm.get_account(&holding_pda(0, &seller.pubkey())).is_none());
    assert_eq!(property_of(&svm, 0).holder_count, 4);
}

#[test]
fn close_rejects_a_holding_still_in_use() {
    let (mut svm, _admin, investors) = finalized_property();
    let cranker = funded(&mut svm);
    fails_with(
        &mut svm,
        close_holding_ix(&cranker.pubkey(), 0, &investors[1].pubkey()),
        &cranker,
        &[&cranker],
        "HoldingNotEmpty",
    );
}
