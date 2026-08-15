//! Rental income: per-mint streams with dust carry, holder claims against
//! checkpoints, the CPI-gated settle hook, and checkpoint teardown.

mod common;
use common::*;

use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token::ID as TOKEN_PROGRAM_ID;

const ASSET: u64 = 7;
const REGION: u16 = 1;
const AGENT_FUNDS: u64 = 1_000_000;

fn tgbp() -> Pubkey {
    Pubkey::new_from_array([8u8; 32])
}
fn tusdc() -> Pubkey {
    Pubkey::new_from_array([9u8; 32])
}

fn give_tokens(svm: &mut LiteSVM, mint: &Pubkey, owner: &Pubkey, amount: u64) {
    set_token_account_for(svm, *mint, token_acc_for(mint, owner), owner, amount);
}

fn balance(svm: &LiteSVM, mint: &Pubkey, owner: &Pubkey) -> u64 {
    balance_at(svm, &token_acc_for(mint, owner))
}

/// Base world: a finalized property with an assigned letting agent funded in
/// both accepted payment mints.
fn income_setup() -> (LiteSVM, Keypair, Keypair) {
    let (mut svm, admin, _authority) = setup();
    set_mint_at(&mut svm, tgbp(), 9);
    set_mint_at(&mut svm, tusdc(), 6);
    seed_market_config(&mut svm, &[tgbp(), tusdc()]);
    seed_location(&mut svm, REGION, POSTCODE);
    seed_property_asset(&mut svm, ASSET, REGION, POSTCODE);
    let agent = new_agent(&mut svm, &admin);
    seed_letting(&mut svm, ASSET, &agent.pubkey());
    give_tokens(&mut svm, &tgbp(), &agent.pubkey(), AGENT_FUNDS);
    give_tokens(&mut svm, &tusdc(), &agent.pubkey(), AGENT_FUNDS);
    (svm, admin, agent)
}

/// A holder with shares and empty token accounts to claim into.
fn holder_with_shares(svm: &mut LiteSVM, admin: &Keypair, shares: u32) -> Keypair {
    let holder = new_holder(svm, admin, ASSET, shares);
    give_tokens(svm, &tgbp(), &holder.pubkey(), 0);
    give_tokens(svm, &tusdc(), &holder.pubkey(), 0);
    holder
}

fn distribute(svm: &mut LiteSVM, agent: &Keypair, mint: &Pubkey, amount: u64) {
    ok(
        svm,
        distribute_ix(&agent.pubkey(), ASSET, mint, amount),
        agent,
        &[agent],
    );
}

fn claim(svm: &mut LiteSVM, holder: &Keypair, mint: &Pubkey) {
    ok(
        svm,
        claim_ix(&holder.pubkey(), ASSET, mint),
        holder,
        &[holder, &sponsor()],
    );
}

#[test]
fn distribute_records_stream_and_funds() {
    let (mut svm, _admin, agent) = income_setup();
    distribute(&mut svm, &agent, &tgbp(), 1_234);

    let income = income_of(&svm, ASSET);
    assert_eq!(income.asset_id, ASSET);
    assert_eq!(income.streams.len(), 1);
    assert_eq!(income.streams[0].mint, tgbp());
    assert_eq!(income.streams[0].per_share, 12);
    assert_eq!(income.streams[0].dust, 34);
    assert_eq!(balance_at(&svm, &income_vault_ata(ASSET, &tgbp())), 1_234);
    assert_eq!(balance(&svm, &tgbp(), &agent.pubkey()), AGENT_FUNDS - 1_234);
}

#[test]
fn distribute_requires_assigned_agent() {
    let (mut svm, admin, _agent) = income_setup();
    let outsider = new_agent(&mut svm, &admin);
    give_tokens(&mut svm, &tgbp(), &outsider.pubkey(), AGENT_FUNDS);
    fails_with(
        &mut svm,
        distribute_ix(&outsider.pubkey(), ASSET, &tgbp(), 100),
        &outsider,
        &[&outsider],
        "NotAssignedAgent",
    );
}

#[test]
fn distribute_rejects_zero_amount() {
    let (mut svm, _admin, agent) = income_setup();
    fails_with(
        &mut svm,
        distribute_ix(&agent.pubkey(), ASSET, &tgbp(), 0),
        &agent,
        &[&agent],
        "ZeroDistribution",
    );
}

#[test]
fn distribute_rejects_foreign_mint() {
    let (mut svm, _admin, agent) = income_setup();
    // XCAV exists as a mint but is not an accepted payment mint.
    give_tokens(&mut svm, &xcav_mint(), &agent.pubkey(), AGENT_FUNDS);
    fails_with(
        &mut svm,
        distribute_ix(&agent.pubkey(), ASSET, &xcav_mint(), 100),
        &agent,
        &[&agent],
        "PaymentMintNotAccepted",
    );
}

#[test]
fn dust_carries_into_the_next_distribution() {
    let (mut svm, _admin, agent) = income_setup();
    distribute(&mut svm, &agent, &tgbp(), 150);
    distribute(&mut svm, &agent, &tgbp(), 149);
    let income = income_of(&svm, ASSET);
    assert_eq!(income.streams[0].per_share, 2);
    assert_eq!(income.streams[0].dust, 99);

    // One more unit tips the carried remainder over a full round.
    distribute(&mut svm, &agent, &tgbp(), 1);
    let income = income_of(&svm, ASSET);
    assert_eq!(income.streams[0].per_share, 3);
    assert_eq!(income.streams[0].dust, 0);
}

#[test]
fn claim_pays_the_holders_share() {
    let (mut svm, admin, agent) = income_setup();
    let holder = holder_with_shares(&mut svm, &admin, 40);
    distribute(&mut svm, &agent, &tgbp(), 1_000);

    claim(&mut svm, &holder, &tgbp());
    assert_eq!(balance(&svm, &tgbp(), &holder.pubkey()), 400);
    assert_eq!(balance_at(&svm, &income_vault_ata(ASSET, &tgbp())), 600);

    let checkpoint = checkpoint_of(&svm, ASSET, &holder.pubkey());
    assert_eq!(checkpoint.entries[0].per_share, 10);
    assert_eq!(checkpoint.entries[0].pending, 0);

    fails_with(
        &mut svm,
        claim_ix(&holder.pubkey(), ASSET, &tgbp()),
        &holder,
        &[&holder, &sponsor()],
        "NothingToClaim",
    );
}

#[test]
fn claim_pays_only_the_delta() {
    let (mut svm, admin, agent) = income_setup();
    let holder = holder_with_shares(&mut svm, &admin, 40);
    distribute(&mut svm, &agent, &tgbp(), 1_000);
    claim(&mut svm, &holder, &tgbp());

    distribute(&mut svm, &agent, &tgbp(), 500);
    claim(&mut svm, &holder, &tgbp());
    assert_eq!(balance(&svm, &tgbp(), &holder.pubkey()), 600);
}

#[test]
fn streams_stay_separate_per_mint() {
    let (mut svm, admin, agent) = income_setup();
    let holder = holder_with_shares(&mut svm, &admin, 40);
    distribute(&mut svm, &agent, &tgbp(), 1_000);
    distribute(&mut svm, &agent, &tusdc(), 700);

    let income = income_of(&svm, ASSET);
    assert_eq!(income.streams.len(), 2);
    assert_eq!(income.streams[0].per_share, 10);
    assert_eq!(income.streams[1].per_share, 7);

    claim(&mut svm, &holder, &tgbp());
    claim(&mut svm, &holder, &tusdc());
    assert_eq!(balance(&svm, &tgbp(), &holder.pubkey()), 400);
    assert_eq!(balance(&svm, &tusdc(), &holder.pubkey()), 280);
}

#[test]
fn locked_shares_still_earn() {
    let (mut svm, admin, agent) = income_setup();
    let holder = funded(&mut svm);
    ok(
        &mut svm,
        roles_assign_ix(&admin.pubkey(), &holder.pubkey(), Role::RealEstateInvestor),
        &admin,
        &[&admin],
    );
    seed_holding_with_lock(&mut svm, ASSET, &holder.pubkey(), 40, 40);
    give_tokens(&mut svm, &tgbp(), &holder.pubkey(), 0);
    distribute(&mut svm, &agent, &tgbp(), 1_000);

    claim(&mut svm, &holder, &tgbp());
    assert_eq!(balance(&svm, &tgbp(), &holder.pubkey()), 400);
}

#[test]
fn claim_needs_an_open_stream() {
    let (mut svm, admin, agent) = income_setup();
    let holder = holder_with_shares(&mut svm, &admin, 40);
    distribute(&mut svm, &agent, &tgbp(), 1_000);
    fails_with(
        &mut svm,
        claim_ix(&holder.pubkey(), ASSET, &tusdc()),
        &holder,
        &[&holder, &sponsor()],
        "UnknownIncomeStream",
    );
}

#[test]
fn claim_rejects_someone_elses_holding() {
    let (mut svm, admin, agent) = income_setup();
    let holder = holder_with_shares(&mut svm, &admin, 40);
    let other = holder_with_shares(&mut svm, &admin, 40);
    distribute(&mut svm, &agent, &tgbp(), 1_000);

    let ix = Instruction::new_with_bytes(
        pid(),
        &property::instruction::ClaimIncome { asset_id: ASSET }.data(),
        property::accounts::ClaimIncome {
            holder: holder.pubkey(),
            payer: sponsor().pubkey(),
            income: income_pda(ASSET),
            checkpoint: checkpoint_pda(ASSET, &holder.pubkey()),
            holding: holding_pda(ASSET, &other.pubkey()),
            payment_mint: tgbp(),
            income_vault: income_vault_pda(ASSET),
            vault_payment_account: income_vault_ata(ASSET, &tgbp()),
            holder_payment: token_acc_for(&tgbp(), &holder.pubkey()),
            payment_token_program: TOKEN_PROGRAM_ID,
            system_program: SYS,
        }
        .to_account_metas(None),
    );
    fails_with(
        &mut svm,
        ix,
        &holder,
        &[&holder, &sponsor()],
        "HoldingMismatch",
    );
}

#[test]
fn claim_rejects_someone_elses_payment_account() {
    let (mut svm, admin, agent) = income_setup();
    let holder = holder_with_shares(&mut svm, &admin, 40);
    let other = holder_with_shares(&mut svm, &admin, 40);
    distribute(&mut svm, &agent, &tgbp(), 1_000);

    let ix = Instruction::new_with_bytes(
        pid(),
        &property::instruction::ClaimIncome { asset_id: ASSET }.data(),
        property::accounts::ClaimIncome {
            holder: holder.pubkey(),
            payer: sponsor().pubkey(),
            income: income_pda(ASSET),
            checkpoint: checkpoint_pda(ASSET, &holder.pubkey()),
            holding: holding_pda(ASSET, &holder.pubkey()),
            payment_mint: tgbp(),
            income_vault: income_vault_pda(ASSET),
            vault_payment_account: income_vault_ata(ASSET, &tgbp()),
            holder_payment: token_acc_for(&tgbp(), &other.pubkey()),
            payment_token_program: TOKEN_PROGRAM_ID,
            system_program: SYS,
        }
        .to_account_metas(None),
    );
    fails_with(
        &mut svm,
        ix,
        &holder,
        &[&holder, &sponsor()],
        "PaymentAccountMismatch",
    );
}

#[test]
fn banked_income_survives_a_closed_holding() {
    let (mut svm, _admin, agent) = income_setup();
    distribute(&mut svm, &agent, &tgbp(), 1_000);

    // No holding account and no role: only a checkpoint with income a
    // settle banked before the holding went away.
    let holder = funded(&mut svm);
    give_tokens(&mut svm, &tgbp(), &holder.pubkey(), 0);
    seed_checkpoint(
        &mut svm,
        ASSET,
        &holder.pubkey(),
        &[CheckpointEntry {
            per_share: 10,
            pending: 123,
        }],
        &sponsor().pubkey(),
    );

    claim(&mut svm, &holder, &tgbp());
    assert_eq!(balance(&svm, &tgbp(), &holder.pubkey()), 123);
}

#[test]
fn settle_rejects_anyone_but_the_marketplace() {
    let (mut svm, admin, agent) = income_setup();
    let holder = holder_with_shares(&mut svm, &admin, 40);
    distribute(&mut svm, &agent, &tgbp(), 1_000);

    // The real gate signature only exists inside a marketplace CPI; any
    // wallet signing in its place must hit the seeds check.
    let impostor = funded(&mut svm);
    fails_with(
        &mut svm,
        settle_ix(
            &impostor.pubkey(),
            &impostor.pubkey(),
            ASSET,
            &holder.pubkey(),
        ),
        &impostor,
        &[&impostor],
        "ConstraintSeeds",
    );
}

#[test]
fn checkpoint_close_needs_an_empty_position() {
    let (mut svm, admin, agent) = income_setup();
    let holder = holder_with_shares(&mut svm, &admin, 40);
    distribute(&mut svm, &agent, &tgbp(), 1_000);
    claim(&mut svm, &holder, &tgbp());

    fails_with(
        &mut svm,
        close_checkpoint_ix(&holder.pubkey(), &sponsor().pubkey(), ASSET),
        &holder,
        &[&holder],
        "SharesStillHeld",
    );
}

#[test]
fn checkpoint_close_refuses_banked_income() {
    let (mut svm, _admin, _agent) = income_setup();
    let holder = funded(&mut svm);
    seed_checkpoint(
        &mut svm,
        ASSET,
        &holder.pubkey(),
        &[CheckpointEntry {
            per_share: 10,
            pending: 5,
        }],
        &sponsor().pubkey(),
    );
    fails_with(
        &mut svm,
        close_checkpoint_ix(&holder.pubkey(), &sponsor().pubkey(), ASSET),
        &holder,
        &[&holder],
        "PendingIncome",
    );
}

#[test]
fn checkpoint_close_returns_the_rent() {
    let (mut svm, _admin, _agent) = income_setup();
    let holder = funded(&mut svm);
    seed_checkpoint(
        &mut svm,
        ASSET,
        &holder.pubkey(),
        &[CheckpointEntry {
            per_share: 10,
            pending: 0,
        }],
        &sponsor().pubkey(),
    );
    ok(
        &mut svm,
        close_checkpoint_ix(&holder.pubkey(), &sponsor().pubkey(), ASSET),
        &holder,
        &[&holder],
    );
    assert!(account_gone(&svm, &checkpoint_pda(ASSET, &holder.pubkey())));
}

#[test]
fn stream_list_is_capped() {
    let (mut svm, _admin, agent) = income_setup();
    let mints: Vec<Pubkey> = (0..9)
        .map(|i| Pubkey::new_from_array([100 + i as u8; 32]))
        .collect();
    seed_market_config(&mut svm, &mints);
    for mint in &mints {
        set_mint_at(&mut svm, *mint, 6);
        give_tokens(&mut svm, mint, &agent.pubkey(), AGENT_FUNDS);
    }
    for mint in &mints[..8] {
        distribute(&mut svm, &agent, mint, 100);
    }
    fails_with(
        &mut svm,
        distribute_ix(&agent.pubkey(), ASSET, &mints[8], 100),
        &agent,
        &[&agent],
        "TooManyIncomeStreams",
    );
}
