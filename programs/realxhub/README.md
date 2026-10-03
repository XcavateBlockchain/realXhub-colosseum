# realXhub

This workspace contains the existing Xcavate property programs and a new `realxhub` program for hub proposals and token purchases. The hub program keeps both flows under one program ID, with separate instruction files for proposals, activation and purchases.

An operator creates a draft, submits it for review, and an authorized verifier approves or rejects the submitted revision. An approved hub can lock XCAV, mint its fixed token supply and open a sale. In legacy paid sales, buyers pay into escrow and claim their tokens when the whole supply sells. If the deadline arrives before sellout, buyers can recover their payments and the operator can recover the XCAV bond.

## What changed

- Added `programs/realxhub`, following the workspace's handler, account, error and event conventions.
- Reused the whitelist's roles and the regions program's ownership records.
- Added per-hub token, payment and bond vaults, plus one purchase position per buyer and hub.
- Added optional unpaid reservations in `src/instructions/reservation.rs`, with separate accounts so existing paid positions and refunds remain readable.
- Added LiteSVM tests that run through actual role grants, region creation, proposal review, minting, purchases and refunds.
- Registered the hub program in `Anchor.toml` and aligned the whitelist address there with its existing Rust declaration.

The existing property programs and their deployment script keep their current behavior.

## Build and test

Use the workspace's Rust toolchain (`rust-toolchain.toml`), Anchor CLI and Solana SBF build tools. This change was checked with Rust 1.89.0, Anchor CLI 1.1.2 and Solana CLI 3.1.10. The committed lockfile resolves Anchor Rust crates to 1.1.2; the workspace's compatible `1.0.0` dependency declarations can still make the CLI print a version warning.

From the repository root:

```sh
anchor build --ignore-keys
cargo test -p realxhub --locked
```

The first command builds all workspace programs, including the whitelist and regions binaries used by the new tests. `--ignore-keys` keeps the build from rewriting source program IDs to match newly generated local deployment keys. It is for building and testing; it does not reconcile deployment identities.

Tests load `target/deploy/*.so` into LiteSVM. They do not need a running validator, a wallet file or SOL. Rebuild after changing program code so tests execute the current binary.

To run the full regression suite and check the new Rust files:

```sh
cargo test --workspace --locked
cargo fmt -p realxhub -- --check
cargo clippy -p realxhub --all-targets --locked -- -D warnings
```

## Configure and use the hub program

Initialize the hub config once, signed by the program's upgrade authority. Supply an XCAV mint, a different payment mint, a verifier public key and a positive bond amount. Amounts use the corresponding mint's smallest units. Both input mints must be classic SPL Token mints without freeze authorities; Token-2022 payment and bond mints are intentionally outside this first version.

Before proposing a hub, initialize the existing whitelist and regions programs, assign a compliant `RegionalOperator` role and create a region owned by that operator. Buyers use the existing compliant `RealEstateInvestor` role for now. Role assignment currently marks a wallet compliant; identity checks happen outside these programs and must precede the grant.

| Instruction | Purpose |
|---|---|
| `initialize_config` | Set the verifier, mints and bond amount |
| `create_proposal` | Create a draft for a region the operator owns |
| `update_proposal` | Edit a draft or rejected application, incrementing its revision |
| `submit_proposal` | Lock the proposal version for review |
| `review_proposal` | Approve/reject that exact revision and metadata hash |
| `activate_hub` | Deposit XCAV, create and mint the full supply, then open the sale |
| `buy_tokens` | Deposit payment and record a token allocation |
| `claim_tokens` | Deliver a buyer's allocation after sellout |
| `finalize_sale` | Mark an unsold sale failed at or after its deadline; anyone may call |
| `refund_purchase` | Return a failed-sale buyer's recorded payment once |
| `refund_bond` | Return a failed-sale operator's recorded XCAV bond once |
| `open_reservations` | Switch an unpurchased listed hub to unpaid reservations |
| `reserve_tokens` | Record quantity and payment quote without transferring assets |
| `claim_reserved_tokens` | Collect payment and deliver the reserved allocation within three days; the final claim releases the first 50% tranche |
| `release_first_tranche` | Release the first tranche once for a reservation hub funded before automatic release was installed; anyone may call, with a fixed operator recipient |
| `initialize_milestone_policy` | Config authority sets the immutable automated assessor, probability threshold and assessment methodology reference |
| `submit_evidence` | Recorded operator submits or replaces progress evidence with a new revision and hash |
| `assess_evidence` | Automated assessor signs a probability score for the current evidence; a qualifying score atomically releases the second tranche |
| `declare_default` | Anyone may mark an unapproved fully funded reservation hub defaulted at or after the 60-day deadline and snapshot recorded payment/bond pools |
| `redeem_default` | Original buyer burns hub tokens to recover proportional payment and XCAV, capped at their paid allocation |
| `reopen_reservations` | Reopen the remaining supply of an expired partially paid round after unpaid promises are cleared |
| `cancel_reservation` | Let a buyer clear their promise before full reservation |
| `release_reservation` | Let anyone clear one expired unpaid promise |
| `finalize_reservations` | Fail an expired, entirely unpaid campaign so its bond can be returned |

`ProposalParams` carries the name, metadata URI/hash, token price, supply and sale duration. The reviewer must check the off-chain documents against that hash and the on-chain terms. The program verifies the reviewer's signature, revision and hash; it does not fetch documents or run AI. A reviewer cannot review their own hub. Rejected proposals require an edit and a new submission. Submitted or approved terms cannot be edited.

Each hub has a numeric ID from `Config.next_hub_id`. The hub PDA uses `["hub", hub_id_le_bytes]`; its mint and three vaults use their named seeds and that ID. Purchase positions use `["position", hub_id_le_bytes, buyer]`. See `programs/realxhub/tests/common/mod.rs` for complete instruction/account builders and `target/idl/realxhub.json` for the generated interface.

For local deployment, use a local validator and deployment keypairs whose public keys match the declared program IDs. Do not deploy using unrelated generated keys or run `anchor keys sync` without reviewing its source changes. This checkout's new hub deployment key is under ignored `target/deploy`; preserve it if keeping that address. A fresh clone needs its own deployment identity, with the source and Anchor configuration updated together. `deploy/deploy.sh` still bootstraps the property system, not the new hub config.

## Reservations and paid claims

After `activate_hub`, a compliant operator who still owns the region may call `open_reservations` before the sale deadline. It succeeds only if no tokens have been purchased or claimed and no payments have been recorded. Existing paid sales continue using `buy_tokens`, `claim_tokens` and their original refunds. Reservation hubs use `Reserving` and `Claiming`, so those paid-sale instructions cannot accidentally collect money in the new flow.

`reserve_tokens` requires a compliant investor, a positive quantity within the remaining supply and a caller-supplied maximum cost. It checks that the buyer's payment account covers the combined quoted payments for that account's unpaid **hub** reservations. It does not transfer stablecoins, deliver hub tokens or change `Hub.total_paid`, `Hub.tokens_sold` or `Hub.tokens_claimed`. Creating accounts still costs SOL rent and transaction fees.

| Account | PDA seeds | What it records |
|---|---|---|
| `ReservationSale` | `["reservation_sale", hub_id_le_bytes]` | Pending unpaid quantity and the three-day window |
| `HubReservation` | `["hub_reservation", hub_id_le_bytes, buyer]` | One buyer's quantity, quoted payment and bound payment account |
| `PaymentReservation` | `["payment_reservation", payment_token_account]` | Total unpaid hub quotes against that payment account |
| `ReservationClaim` | `["reservation_claim", hub_id_le_bytes, buyer]` | Cumulative paid and delivered quantity, payment and latest claim timestamp |
| `HubFunding` | `["funding", hub_id_le_bytes]` | First tranche amount, release flag and release timestamp |
| `MilestonePolicy` | `["milestone_policy"]` | Immutable assessor, approval threshold and methodology URI/hash |
| `HubMilestone` | `["milestone", hub_id_le_bytes]` | Current evidence revision, deadline, signed assessment and second-tranche receipt |
| `HubDefault` | `["default", hub_id_le_bytes]` | Fixed recorded pools, approval/default timestamps and aggregate redeemed quantities |
| `DefaultRedemption` | `["default_redemption", hub_id_le_bytes, buyer]` | Cumulative original-buyer token surrender and both asset payouts |

The buyer may add to the same reservation using its original payment account. Cancellation clears the whole reservation and its quote; an eligible buyer can reserve again while the sale is open. These records stay allocated, so cancellation does not recover account rent. The hub ledger does not include reservations held by the separate marketplace program.

The last available token being reserved sets `Claiming`, `claim_started_at` and `claim_deadline` in that transaction. The deadline is exactly 259,200 seconds later, even if the reservation arrives just before the original sale deadline. Further reservations and cancellation cannot reset or reopen that round. Deadline boundaries use Solana's on-chain clock: entry requires `now < deadline`, expiry allows `now >= deadline`.

Anyone can call `release_reservation` after the sale deadline for a partially reserved hub, or after the three-day deadline for a fully reserved hub. It deducts only that buyer's recorded quantity and quote, without moving funds. Cancellation and cleanup have no role gate, so eligibility revocation cannot strand an old promise. `finalize_reservations` marks an expired campaign failed only when no paid or delivered allocations exist. The existing `refund_bond` then returns the operator's recorded bond once. Cleanup may run before or after finalization.

Reservation quotes are promises, not escrow or proof of future payment. Buyers can spend or close their payment accounts after reserving. `claim_reserved_tokens(hub_id, max_total_cost)` requires current investor compliance and `now < claim_deadline`. It collects the full quoted payment from the original bound account and delivers the entire reserved quantity to the buyer's hub-token ATA in one transaction. A failed payment or delivery rolls back both transfers, account initialization and every counter.

A successful claim clears that buyer's unpaid quantity and aggregate quote, records a separate `ReservationClaim`, and increments `Hub.total_paid`, `Hub.tokens_sold` and `Hub.tokens_claimed`. The last claim sets `Funded`, releases the first tranche and emits `ReservationFundingCompleted`. A buyer may claim again if they make another reservation in a reopened round; the receipt accumulates actual payments and delivered tokens. Legacy paid purchase positions remain separate.

If only some buyers claim within three days, anyone can use `release_reservation` to clear each expired unpaid promise and then call `reopen_reservations`. Reopening requires all pending unpaid quantities to be zero, preserves delivered tokens and escrowed payments, and starts a new reservation period lasting the hub's original `sale_duration`. Only `token_supply - tokens_sold` is available. Reserving all remaining tokens starts a fresh three-day claim window. Clearing old promises before reopening prevents an old unpaid reservation from becoming valid under the new deadline.

A reopened reservation period that expires without filling can be cleaned up and renewed in the same way. Partially paid campaigns cannot use `finalize_reservations` or `refund_bond`; their remaining allocations reopen rather than refunding payments or returning collateral. Campaigns with no paid claims retain the existing failed-sale and bond-refund path. Cleanup and reopening require transactions; time passing alone does not execute them. Partial campaigns cannot release a funding tranche.

### First funding tranche

The final `claim_reserved_tokens` transaction automatically transfers `floor(Hub.total_paid / 2)` payment units from that hub's payment vault to the recorded operator's canonical payment-mint ATA. It creates the ATA if needed, with rent paid by the final buyer. The amount comes from recorded payments, so unsolicited vault donations do not enlarge it. If the target has an odd number of payment units, the extra unit stays in escrow for the second tranche. The entire remaining payment and XCAV bond stay in their separate hub vaults.

A `HubFunding` receipt records the release exactly once, including zero-unit first tranches for a one-unit target. If the payout fails, the final buyer's payment, token delivery, account creation, counters and funded status all roll back. Replayed claims and separate payout calls cannot release it again. Existing hub, purchase and reservation account layouts are unchanged.

`release_first_tranche(hub_id)` supports reservation hubs that were already fully paid and claimed under the earlier version. Any signer can pay transaction/account costs, but the recipient remains the recorded operator's payment ATA. It requires `Funded`, the full supply sold and claimed, exact recorded payment and no pending unpaid reservations or refunds. It shares the automatic claim's receipt and replay guard. Legacy paid sales keep their existing behavior.

Claim instruction builders must now supply the operator, operator payment ATA and funding receipt. The generated IDL groups the buyer's claim and hub funding records under `records`, with the same buyer signer, hub and system program repeated there. The program checks that the grouped buyer and hub match the paying buyer and validated hub.

### Progress evidence and second tranche

The config authority calls `initialize_milestone_policy` once to set an automated assessor public key, approval threshold and assessment-policy URI/hash. The threshold is an explicitly supplied integer from 1 to 10,000 basis points, with 10,000 representing 100%; there is no production default. The URI/hash identifies the off-chain methodology, model version and criteria the assessor must use. The account has no update or rotation instruction, so callers cannot change the threshold, signer or methodology while a hub is awaiting assessment. Proposal review keeps its separate configured verifier and instructions.

The recorded operator calls `submit_evidence(hub_id, EvidenceParams)` after all tokens have been paid for and claimed and the first tranche has been released. Evidence carries a nonempty URI of at most 200 UTF-8 bytes and a nonzero content hash. Each submission increments the evidence revision. Pending or rejected evidence can be replaced; a replacement clears the previous assessment and invalidates any signed score for the older revision. The latest revision lives in the account; submission/assessment events retain the revision and hashes for an external audit index.

The approval deadline is exactly 5,184,000 seconds (60 days) after `HubFunding.first_tranche_released_at`. Submitting or replacing evidence does not reset it. Both submission and assessment require `now < deadline`. An on-time submission approved at or after the deadline does not release payment. This version has no assessment grace period or outage extension.

The automated service calls `assess_evidence(hub_id, AssessmentParams)` and signs as the immutable policy's assessor. Its parameters bind the exact evidence revision and hash, assessment-policy hash, probability in basis points and assessment-report URI/hash. The program checks this signature and all references; it computes approval as `probability_bps >= approval_threshold_bps`. There is no boolean override or community vote. A hub operator cannot assess their own hub. A low score records `Rejected` without releasing funds; changing that decision requires a new operator evidence submission.

Approval immediately transfers `Hub.total_paid - HubFunding.first_tranche_amount` to the recorded operator's canonical payment-mint ATA. The service pays to recreate that ATA if it was closed. An odd payment target's extra base unit is included in this second tranche; unsolicited vault donations are excluded. The approval and second-tranche receipt commit only if the payment succeeds, so a failed transfer can be retried without a partially approved state. The receipt prevents repeated payout and further evidence replacement. Buyer tokens, first-tranche records and the XCAV bond are untouched; existing account layouts and claim instruction accounts are unchanged by this step.

**Automation integration:** This repository implements the program-side submission, signed-score authorization and payout. It does not contain or deploy a scoring worker, fetch evidence documents, verify their bytes against the submitted hash, or calculate a probability of success. The assessor service must perform those tasks under the published methodology, maintain its signing key and submit the transaction before the deadline. The program trusts that service's attested score; a service-key signature alone does not prove that a model ran or that the probability is calibrated. The service, methodology and numerical threshold must be selected and the policy initialized before this integration can run automatically. The 7,000-basis-point threshold used in tests is test data, not a recommendation or production configuration.

A service integration should read the current `HubMilestone` and `MilestonePolicy`, verify the evidence content hash, evaluate it under the policy's methodology, persist a report with its content hash, then submit the corresponding score and exact current revision. A replaced submission causes stale transactions to fail; a payout failure leaves the submission pending for retry. The generated IDL provides the instruction/account builders.

### Default settlement and token surrender

`declare_default(hub_id)` is permissionless at or after the exact 60-day approval deadline. It requires full payment and token delivery, the first tranche receipt and an unrefunded recorded bond. The canonical milestone address must be supplied even if no evidence was submitted; if it exists, its owner, hub, deadline and absence of approval are checked. A successful second tranche blocks default. No evidence, pending evidence, rejected evidence or an absent assessor policy does not prevent an otherwise eligible default.

Declaration appends the `Defaulted` hub phase and creates `HubDefault`. Its pools are `Hub.total_paid - HubFunding.first_tranche_amount` and `Hub.bond_amount`, with sufficient escrow required. Current vault balances and donations do not enlarge those entitlements. Declaration transfers nothing. It permanently disables the funded evidence/payout path and cannot use the failed-sale operator bond refund.

`redeem_default(hub_id, amount, min_payment_out, min_xcav_out)` requires the original buyer's signature and paid `ReservationClaim`, but no current compliance role. The buyer must hold the surrendered quantity in an owned account for that hub mint. The transaction permanently burns those tokens and transfers both proportional assets to the buyer's canonical ATAs, creating them if needed. Burn, both transfers, account creation and receipts roll back together on failure. The caller supplies minimum acceptable outputs.

Partial redemptions are allowed. Cumulative surrendered quantity cannot exceed that buyer's original paid allocation, including purchases across reopened rounds. A secondary holder without an original paid receipt cannot redeem. An original buyer who transferred tokens away must reacquire tokens to surrender; buying someone else's tokens does not increase their original allocation limit.

For pool `P`, original fixed supply `S`, previous surrendered quantity `q0` and new cumulative quantity `q1`, a nonfinal payout is `floor(P*q1/S) - floor(P*q0/S)` using `u128` multiplication. Splitting calls cannot increase the normal proportional entitlement. The transaction redeeming the final outstanding token receives the remaining base-unit rounding dust, so the full recorded pools can be distributed. This small dust allocation depends on redemption order. Unclaimed entitlements stay escrowed if a buyer never redeems or cannot recover tokens to burn; there is no alternate lost-token claim.

Default settlement must be invoked through transactions; time passing does not execute declaration or payouts. The fixed XCAV bond already locked is redeemable, but **oracle valuation at 30% and maintaining that value with top ups remain deferred**. No feed has been configured or price assumed. Successful-hub bond release is also still absent. This workspace does not include the assessor worker, frontend or backend.

To check this part alone after building the workspace binaries:

```sh
anchor build -p realxhub --ignore-keys
cargo test -p realxhub --test reservation --test reservation_claim --test milestone --test default --locked
```

The reservation tests exercise the compiled program, including unchanged balances before payment, window timing, multiple buyers, quotes across hubs, payment-account binding, atomic payment and delivery rollback, duplicate claims, role revocation, expiry cleanup, reopening, cumulative receipts and the existing unpaid bond-refund path. Tranche tests cover final-claim release, fixed recipients, replay protection, odd payment units, donations, payout rollback and previously funded reservation hubs. Default tests cover exact expiry, missing/pending/rejected evidence, successful approval exclusion, original-buyer eligibility, secondary transfers, allocation caps, compliance revocation, atomic burn and dual payout rollback, cumulative rounding, final dust, donations and account substitution. Milestone tests cover immutable authority-set policy, exact score boundaries, operator/assessor authorization, evidence revision binding, deadline boundaries, rejection and resubmission, atomic payout retries, recipient recovery and preserved bonds/tokens.

## Rules and current scope

- XCAV is bonded **after approval**. A rejected application has not paid XCAV and has nothing to refund. The bond amount is recorded when its draft is created, and activation accepts a caller-supplied maximum.
- Sales are fixed price. The funding target is `token_price * token_supply`, computed with checked arithmetic. Legacy paid sales are all-or-nothing; reservation buyers receive tokens when they pay, and partial claim rounds reopen the remaining supply. Both payment instructions accept a maximum cost and reject payment at the exact applicable deadline.
- Tokens have zero decimals and a fixed supply. Minting authority is removed at activation, and there is no freeze authority. They are standard transferable SPL tokens once claimed; no ownership, revenue or governance rights are implied by this implementation.
- A legacy paid purchase allocates tokens; it does not deliver transferable tokens before sellout. Failed-sale refunds do not require recovering tokens from buyers. Transaction fees and account rent are not part of payment refunds.
- Payments and bonds use separate vaults for each hub. Refund amounts come from recorded liabilities, not the current vault balance. Eligibility revocation does not block an existing refund or funded-sale token claim.
- Fully paid and claimed reservation hubs release the first 50% payment tranche once, then release the remaining recorded payment when the configured assessor attests a qualifying evidence score before the 60-day deadline. If no approval arrives, a declared default allows original buyers to burn tokens for shares of the remaining payment and recorded XCAV bond. Successful hub bonds and legacy paid-sale payments remain in escrow. Automated scoring, oracle-based 30% valuation and top ups, and successful-hub bond release are future work.
- Config is fixed after initialization. Verifier rotation, cancelled-sale token cleanup and account-rent recovery are also future work. There is no frontend in this change.

This is the first proposal and purchase milestone. Define the release conditions and hub token rights before using it for real funding.
