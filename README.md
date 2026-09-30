# realXhub

This Anchor workspace contains the existing Xcavate property programs and a new `realxhub` program that combines hub proposals and token purchases.

Operators create a draft and submit a specific revision for review. An authorized verifier approves or rejects it. Approved hubs can bond XCAV, mint a fixed token supply and open a sale. Buyers pay into escrow and claim tokens after sellout. An expired partial sale refunds buyers and returns the operator's bond.

## Changes

- Added proposal, activation and purchase handlers in `programs/realxhub`, following the existing Rust modules, account constraints, errors and events.
- Reused whitelist roles and region ownership, with separate vaults and purchase records for each hub.
- Added LiteSVM tests for the full journey and rejected actions.
- Registered the program in `Anchor.toml` and corrected the whitelist address there to match its Rust declaration.
- Gave the existing secondary-sale income test an explicit compute budget so random PDA derivations cannot intermittently exhaust its default budget.

## Build and test

Use the toolchain in `rust-toolchain.toml`, Anchor CLI and Solana SBF build tools. This change was checked with Rust 1.89.0, Anchor CLI 1.1.2 and Solana CLI 3.1.10.

Run from the repository root:

```sh
anchor build --ignore-keys
cargo test -p realxhub --locked
cargo test --workspace --locked
cargo fmt -p realxhub -- --check
cargo clippy -p realxhub --all-targets --locked -- -D warnings
```

Tests load the built `.so` files into LiteSVM; they do not need a validator or wallet. Rebuild after changing program code. `--ignore-keys` prevents build-time program ID rewrites; deployment still requires keys matching the declared IDs. The CLI may warn about the compatible `1.0.0` dependency declarations, although the lockfile resolves Anchor crates to 1.1.2.

## Configuration and scope

Initialize the hub config using the program's upgrade authority. Choose a verifier, positive XCAV bond amount and two different classic SPL Token mints without freeze authorities: XCAV and the payment token. Prices and bonds use each mint's smallest units.

Operators need a compliant `RegionalOperator` role and must own the chosen region. Buyers currently reuse the compliant `RealEstateInvestor` role. Identity screening and document review happen off chain; the program records an authorized review tied to the submitted revision and hash.

This first version bonds XCAV after approval and uses fixed-price, all-or-nothing sales. Tokens are standard transferable SPL tokens after claiming; their economic and governance rights are not defined here.

Successful-sale payments and bonds remain in escrow. Milestone payouts, voting, buybacks, slashing, verifier rotation and a frontend are later work. The existing deployment script does not initialize the new hub program.

See [the hub program guide](programs/realxhub/README.md) for instructions, account derivations, deployment notes and the detailed rules.
