//! Share-mint creation: the Token-2022 mint with its extensions, the supply
//! landing in the property vault, and the guards on the second listing step.

mod common;
use common::*;

use anchor_spl::token_2022::spl_token_2022::{
    extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions},
    state::{Account as TokenAccountState, AccountState, Mint as MintState},
};
use marketplace::state::ListingStatus;

/// Full setup with region 1 + location, a compliant developer, and listing 0
/// created (still `PendingAssets`).
fn setup_pending() -> (LiteSVM, Keypair, Keypair, Keypair) {
    let (mut svm, admin, authority) = setup();
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
    (svm, admin, authority, developer)
}

#[test]
fn init_assets_mints_supply_to_vault() {
    let (mut svm, _admin, _authority, developer) = setup_pending();
    ok(
        &mut svm,
        init_assets_ix(&developer.pubkey(), 0),
        &developer,
        &[&developer],
    );

    assert_eq!(listing_of(&svm, 0).status, ListingStatus::Listed);
    assert_eq!(property_of(&svm, 0).share_mint, share_mint_pda(0));

    // The mint: 0 decimals, full supply, authorities on the mint-auth PDA.
    let mint_acc = svm.get_account(&share_mint_pda(0)).unwrap();
    assert_eq!(mint_acc.owner, anchor_spl::token_2022::ID);
    let mint = StateWithExtensions::<MintState>::unpack(&mint_acc.data).unwrap();
    assert_eq!(mint.base.decimals, 0);
    assert_eq!(mint.base.supply, SHARE_AMOUNT as u64);
    // The mint authority is given up once the supply is minted, so the cap is
    // verifiable on chain; the program keeps only the account-state authority.
    assert!(mint.base.mint_authority.is_none());
    assert_eq!(mint.base.freeze_authority.unwrap(), mint_auth_pda(0));
    let extensions = mint.get_extension_types().unwrap();
    assert!(extensions.contains(&ExtensionType::DefaultAccountState));
    assert!(extensions.contains(&ExtensionType::PermanentDelegate));

    // The vault's share account holds the supply and is open for the program
    // to deliver from (thawed once at creation).
    let vault_acc = svm.get_account(&vault_share_account(0)).unwrap();
    let token = StateWithExtensions::<TokenAccountState>::unpack(&vault_acc.data).unwrap();
    assert_eq!(token.base.amount, SHARE_AMOUNT as u64);
    assert_eq!(token.base.owner, property_vault_pda(0));
    assert_eq!(token.base.state, AccountState::Initialized);
}

// Anyone can send lamports to the predictable mint PDA before the developer
// gets to it; creation must survive that instead of bricking the listing.
#[test]
fn init_assets_survives_prefunded_mint_pda() {
    let (mut svm, _admin, _authority, developer) = setup_pending();
    svm.airdrop(&share_mint_pda(0), 1).unwrap();

    ok(
        &mut svm,
        init_assets_ix(&developer.pubkey(), 0),
        &developer,
        &[&developer],
    );
    assert_eq!(listing_of(&svm, 0).status, ListingStatus::Listed);
}

#[test]
fn init_assets_requires_listing_developer() {
    let (mut svm, admin, _authority, _developer) = setup_pending();
    let other = new_developer(&mut svm, &admin);
    fails_with(
        &mut svm,
        init_assets_ix(&other.pubkey(), 0),
        &other,
        &[&other],
        "NotListingDeveloper",
    );
}

#[test]
fn init_assets_twice_fails() {
    let (mut svm, _admin, _authority, developer) = setup_pending();
    ok(
        &mut svm,
        init_assets_ix(&developer.pubkey(), 0),
        &developer,
        &[&developer],
    );
    // The listing is no longer PendingAssets.
    fails_with(
        &mut svm,
        init_assets_ix(&developer.pubkey(), 0),
        &developer,
        &[&developer],
        "ListingNotActive",
    );
}

#[test]
fn init_assets_rejects_after_expiry() {
    let (mut svm, _admin, _authority, developer) = setup_pending();
    warp(&mut svm, LISTING_DURATION + 1);
    fails_with(
        &mut svm,
        init_assets_ix(&developer.pubkey(), 0),
        &developer,
        &[&developer],
        "ListingExpired",
    );
}

// The compliance gate spans both listing steps: a developer revoked between
// step one and step two can't open the sale.
#[test]
fn init_assets_requires_compliant_developer() {
    let (mut svm, admin, _authority, developer) = setup_pending();
    set_permission(
        &mut svm,
        &admin,
        &developer.pubkey(),
        Role::RealEstateDeveloper,
        false,
    );
    fails_with(
        &mut svm,
        init_assets_ix(&developer.pubkey(), 0),
        &developer,
        &[&developer],
        "NotCompliant",
    );
}

// --- Core asset and region collections ---

#[test]
fn init_assets_mints_the_property_deed() {
    let (mut svm, _admin, _sponsor, developer) = setup_pending();
    ok(
        &mut svm,
        init_assets_ix(&developer.pubkey(), 0),
        &developer,
        &[&developer],
    );

    assert_eq!(property_of(&svm, 0).core_asset, core_asset_pda(0));
    let acc = svm.get_account(&core_asset_pda(0)).unwrap();
    assert_eq!(acc.owner, mpl_core::ID);
    let asset = mpl_core::accounts::BaseAssetV1::from_bytes(&acc.data).unwrap();
    assert_eq!(asset.owner, property_vault_pda(0));
    assert_eq!(
        asset.update_authority,
        mpl_core::types::UpdateAuthority::Collection(core_collection_pda(1))
    );
    assert_eq!(asset.name, "10 Test Street");
    assert_eq!(asset.uri, "ipfs://property-docs");
}

#[test]
fn init_assets_needs_the_region_collection() {
    let (mut svm, _admin, _sponsor, developer) = setup_pending();
    // Blank out the collection the region seeding provided.
    svm.set_account(
        core_collection_pda(1),
        solana_account::Account {
            lamports: 0,
            data: vec![],
            owner: SYS,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    fails_with(
        &mut svm,
        init_assets_ix(&developer.pubkey(), 0),
        &developer,
        &[&developer],
        "CollectionMissing",
    );
}

#[test]
fn init_assets_validates_the_metadata() {
    let (mut svm, _admin, _sponsor, developer) = setup_pending();
    fails_with(
        &mut svm,
        init_assets_ix_full(&developer.pubkey(), 0, 1, "".into(), "ipfs://x".into()),
        &developer,
        &[&developer],
        "InvalidAssetMetadata",
    );
    fails_with(
        &mut svm,
        init_assets_ix_full(&developer.pubkey(), 0, 1, "x".repeat(41), "ipfs://x".into()),
        &developer,
        &[&developer],
        "InvalidAssetMetadata",
    );
    fails_with(
        &mut svm,
        init_assets_ix_full(&developer.pubkey(), 0, 1, "ok".into(), "u".repeat(201)),
        &developer,
        &[&developer],
        "InvalidAssetMetadata",
    );
}

#[test]
fn create_region_collection_is_authority_only() {
    let (mut svm, admin, _authority) = setup();
    let operator = funded(&mut svm);
    // A bare region: no collection yet.
    seed_region_bare(&mut svm, 5, &operator.pubkey(), 300);

    fails_with(
        &mut svm,
        create_collection_ix(&admin.pubkey(), 5, "region five", "ipfs://region"),
        &admin,
        &[&admin],
        "NotAuthority",
    );
}

#[test]
fn create_region_collection_creates_and_refuses_twice() {
    let (mut svm, _admin, authority) = setup();
    let operator = funded(&mut svm);
    seed_region_bare(&mut svm, 5, &operator.pubkey(), 300);

    ok(
        &mut svm,
        create_collection_ix(&authority.pubkey(), 5, "region five", "ipfs://region"),
        &authority,
        &[&authority],
    );
    let acc = svm.get_account(&core_collection_pda(5)).unwrap();
    let collection = mpl_core::accounts::BaseCollectionV1::from_bytes(&acc.data).unwrap();
    assert_eq!(collection.update_authority, core_auth_pda());
    assert_eq!(collection.name, "region five");
    assert_eq!(collection.uri, "ipfs://region");

    // The Core program refuses to create over a live collection.
    fails_with(
        &mut svm,
        create_collection_ix(&authority.pubkey(), 5, "again", "ipfs://again"),
        &authority,
        &[&authority],
        "already in use",
    );
}

#[test]
fn create_region_collection_needs_the_region() {
    let (mut svm, _admin, authority) = setup();
    fails_with(
        &mut svm,
        create_collection_ix(&authority.pubkey(), 9, "ghost", "ipfs://ghost"),
        &authority,
        &[&authority],
        "AccountNotInitialized",
    );
}

/// The deed CPIs are hand-encoded (the mpl-core crate stays out of the
/// on-chain build); these pin every byte and meta against the crate's own
/// builders, so upstream drift fails here instead of on devnet.
#[test]
fn deed_encodings_match_the_mpl_core_crate() {
    let a = Pubkey::new_unique();
    let b = Pubkey::new_unique();
    let c = Pubkey::new_unique();
    let d = Pubkey::new_unique();
    let e = Pubkey::new_unique();

    let expected = mpl_core::instructions::CreateCollectionV2Builder::new()
        .collection(a)
        .update_authority(Some(b))
        .payer(c)
        .name("n".into())
        .uri("u".into())
        .instruction();
    assert_eq!(
        marketplace::deed::create_collection_data("n", "u").unwrap(),
        expected.data
    );
    assert_eq!(
        marketplace::deed::create_collection_metas(a, b, c),
        expected.accounts
    );

    let expected = mpl_core::instructions::CreateV2Builder::new()
        .asset(a)
        .collection(Some(b))
        .authority(Some(c))
        .payer(d)
        .owner(Some(e))
        .name("n".into())
        .uri("u".into())
        .instruction();
    assert_eq!(
        marketplace::deed::create_asset_data("n", "u").unwrap(),
        expected.data
    );
    assert_eq!(
        marketplace::deed::create_asset_metas(a, b, c, d, e),
        expected.accounts
    );

    let expected = mpl_core::instructions::BurnV1Builder::new()
        .asset(a)
        .collection(Some(b))
        .payer(c)
        .authority(Some(d))
        .instruction();
    assert_eq!(marketplace::deed::burn_asset_data(), expected.data);
    assert_eq!(
        marketplace::deed::burn_asset_metas(a, b, c, d),
        expected.accounts
    );
}
