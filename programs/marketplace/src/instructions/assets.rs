use anchor_lang::prelude::*;
use anchor_lang::system_program::{
    allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer,
};
use anchor_spl::associated_token::{create as create_ata, AssociatedToken, Create as CreateAta};
use anchor_spl::token_2022::spl_token_2022::instruction::AuthorityType;
use anchor_spl::token_2022::spl_token_2022::{
    extension::ExtensionType, state::AccountState, state::Mint as MintState,
};
use anchor_spl::token_2022::{
    initialize_mint2, mint_to, set_authority, thaw_account, InitializeMint2, MintTo, SetAuthority,
    ThawAccount, Token2022,
};
use anchor_spl::token_2022_extensions::{
    default_account_state_initialize, mint_close_authority_initialize,
    permanent_delegate_initialize, DefaultAccountStateInitialize, MintCloseAuthorityInitialize,
    PermanentDelegateInitialize,
};

use crate::constants::{
    CONFIG_SEED, CORE_ASSET_SEED, CORE_AUTH_SEED, CORE_COLLECTION_SEED, LISTING_SEED,
    MINT_AUTH_SEED, MPL_CORE_PROGRAM, PROPERTY_SEED, PROPERTY_VAULT_SEED, SHARE_MINT_SEED,
};
use crate::deed;
use crate::error::MarketplaceError;
use crate::state::{Config, Listing, ListingStatus, PropertyAsset};

use xcavate_whitelist::state::{Role, RoleAccount};

/// Second half of listing a property: creates the Token-2022 share mint and
/// mints the whole supply into the property vault, then opens the listing for
/// purchases. Split from `list_property` because the mint creation doesn't fit
/// the same transaction. Only the listing's developer may call it.
///
/// The mint is 0-decimals with supply = share amount. Holder token accounts
/// start non-transferable (`default_account_state`), and the mint-authority
/// PDA is the permanent delegate, so the program moves shares itself and a
/// holder can never transfer them directly.
#[derive(Accounts)]
#[instruction(listing_id: u64)]
pub struct InitPropertyAssets<'info> {
    #[account(mut)]
    pub developer: Signer<'info>,

    /// The caller's RealEstateDeveloper role. The whole two-step listing is
    /// one compliance-gated flow, so the second step re-checks the flag; a
    /// developer revoked between the steps can't open the sale.
    #[account(
        seeds = [
            xcavate_whitelist::ROLE_SEED,
            developer.key().as_ref(),
            &[Role::RealEstateDeveloper.seed_byte()],
        ],
        bump = developer_role.bump,
        seeds::program = xcavate_whitelist::ID,
        constraint = developer_role.is_compliant() @ MarketplaceError::NotCompliant,
    )]
    pub developer_role: Box<Account<'info, RoleAccount>>,

    #[account(
        mut,
        seeds = [LISTING_SEED, &listing_id.to_le_bytes()],
        bump = listing.bump,
        constraint = listing.developer == developer.key() @ MarketplaceError::NotListingDeveloper,
    )]
    pub listing: Box<Account<'info, Listing>>,

    #[account(
        mut,
        seeds = [PROPERTY_SEED, &listing_id.to_le_bytes()],
        bump = property.bump,
    )]
    pub property: Box<Account<'info, PropertyAsset>>,

    /// CHECK: the share mint PDA, created and initialized in the handler.
    #[account(
        mut,
        seeds = [SHARE_MINT_SEED, &listing_id.to_le_bytes()],
        bump,
    )]
    pub share_mint: UncheckedAccount<'info>,

    /// CHECK: mint + freeze authority and permanent delegate of the share
    /// mint; a bare PDA the program signs with.
    #[account(seeds = [MINT_AUTH_SEED, &listing_id.to_le_bytes()], bump)]
    pub mint_auth: UncheckedAccount<'info>,

    /// CHECK: the property vault authority; a bare PDA that owns the
    /// property's token accounts.
    #[account(seeds = [PROPERTY_VAULT_SEED, &listing_id.to_le_bytes()], bump)]
    pub property_vault: UncheckedAccount<'info>,

    /// CHECK: the vault's associated token account for the share mint; the
    /// associated-token program verifies the derivation when creating it.
    #[account(mut)]
    pub vault_share_account: UncheckedAccount<'info>,

    /// CHECK: the property's Core asset PDA; the Core program creates it.
    #[account(mut, seeds = [CORE_ASSET_SEED, &listing_id.to_le_bytes()], bump)]
    pub core_asset: UncheckedAccount<'info>,

    /// CHECK: the region's Core collection, created up front by the
    /// authority; the seeds tie it to the property's region.
    #[account(
        mut,
        seeds = [CORE_COLLECTION_SEED, &property.region_id.to_le_bytes()],
        bump,
    )]
    pub core_collection: UncheckedAccount<'info>,

    /// CHECK: update authority of every region collection; a bare PDA the
    /// program signs asset creation with.
    #[account(seeds = [CORE_AUTH_SEED], bump)]
    pub core_auth: UncheckedAccount<'info>,

    /// CHECK: the Metaplex Core program, pinned by address.
    #[account(address = MPL_CORE_PROGRAM @ MarketplaceError::WrongProgram)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// Longest name and metadata URI a property's Core asset may carry.
pub const MAX_ASSET_NAME_LEN: usize = 40;
pub const MAX_ASSET_URI_LEN: usize = 200;

pub fn init_property_assets_handler(
    ctx: Context<InitPropertyAssets>,
    listing_id: u64,
    name: String,
    uri: String,
) -> Result<()> {
    let listing = &ctx.accounts.listing;
    require!(
        listing.status == ListingStatus::PendingAssets,
        MarketplaceError::ListingNotActive
    );
    require!(
        Clock::get()?.unix_timestamp < listing.listing_expiry,
        MarketplaceError::ListingExpired
    );
    require!(
        !name.is_empty() && name.len() <= MAX_ASSET_NAME_LEN,
        MarketplaceError::InvalidAssetMetadata
    );
    require!(
        !uri.is_empty() && uri.len() <= MAX_ASSET_URI_LEN,
        MarketplaceError::InvalidAssetMetadata
    );

    let id_bytes = listing_id.to_le_bytes();
    let mint_seeds: &[&[u8]] = &[SHARE_MINT_SEED, &id_bytes, &[ctx.bumps.share_mint]];
    let auth_seeds: &[&[u8]] = &[MINT_AUTH_SEED, &id_bytes, &[ctx.bumps.mint_auth]];

    // Create the mint account sized for its three extensions, then
    // initialize the extensions (they must precede the mint itself).
    let space = ExtensionType::try_calculate_account_len::<MintState>(&[
        ExtensionType::DefaultAccountState,
        ExtensionType::PermanentDelegate,
        ExtensionType::MintCloseAuthority,
    ])?;
    let rent = Rent::get()?.minimum_balance(space);
    if ctx.accounts.share_mint.lamports() == 0 {
        create_account(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.key(),
                CreateAccount {
                    from: ctx.accounts.developer.to_account_info(),
                    to: ctx.accounts.share_mint.to_account_info(),
                },
                &[mint_seeds],
            ),
            rent,
            space as u64,
            ctx.accounts.token_program.key,
        )?;
    } else {
        // The PDA holds lamports already; anyone can send them, and
        // create_account fails on a funded account. Claim it piecewise so a
        // 1-lamport transfer can't brick the listing.
        let deficit = rent.saturating_sub(ctx.accounts.share_mint.lamports());
        if deficit > 0 {
            transfer(
                CpiContext::new(
                    ctx.accounts.system_program.key(),
                    Transfer {
                        from: ctx.accounts.developer.to_account_info(),
                        to: ctx.accounts.share_mint.to_account_info(),
                    },
                ),
                deficit,
            )?;
        }
        allocate(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.key(),
                Allocate {
                    account_to_allocate: ctx.accounts.share_mint.to_account_info(),
                },
                &[mint_seeds],
            ),
            space as u64,
        )?;
        assign(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.key(),
                Assign {
                    account_to_assign: ctx.accounts.share_mint.to_account_info(),
                },
                &[mint_seeds],
            ),
            ctx.accounts.token_program.key,
        )?;
    }
    default_account_state_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            DefaultAccountStateInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.share_mint.to_account_info(),
            },
        ),
        &AccountState::Frozen,
    )?;
    permanent_delegate_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            PermanentDelegateInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.share_mint.to_account_info(),
            },
        ),
        ctx.accounts.mint_auth.key,
    )?;
    // The program can close the mint at teardown, so a dead property's rent
    // isn't stranded forever.
    mint_close_authority_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            MintCloseAuthorityInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.share_mint.to_account_info(),
            },
        ),
        Some(ctx.accounts.mint_auth.key),
    )?;
    initialize_mint2(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            InitializeMint2 {
                mint: ctx.accounts.share_mint.to_account_info(),
            },
        ),
        0,
        ctx.accounts.mint_auth.key,
        Some(ctx.accounts.mint_auth.key),
    )?;

    // The vault's share account, then the full supply into it. The account
    // starts frozen like every other, so it is thawed once and stays open:
    // only the program's own PDA can sign transfers out of it anyway.
    create_ata(CpiContext::new(
        ctx.accounts.associated_token_program.key(),
        CreateAta {
            payer: ctx.accounts.developer.to_account_info(),
            associated_token: ctx.accounts.vault_share_account.to_account_info(),
            authority: ctx.accounts.property_vault.to_account_info(),
            mint: ctx.accounts.share_mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        },
    ))?;
    thaw_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        ThawAccount {
            account: ctx.accounts.vault_share_account.to_account_info(),
            mint: ctx.accounts.share_mint.to_account_info(),
            authority: ctx.accounts.mint_auth.to_account_info(),
        },
        &[auth_seeds],
    ))?;
    mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            MintTo {
                mint: ctx.accounts.share_mint.to_account_info(),
                to: ctx.accounts.vault_share_account.to_account_info(),
                authority: ctx.accounts.mint_auth.to_account_info(),
            },
            &[auth_seeds],
        ),
        ctx.accounts.property.share_amount as u64,
    )?;
    // The supply is fixed at listing time, so give up the mint authority right
    // away; that makes the cap verifiable from the mint account alone.
    set_authority(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            SetAuthority {
                current_authority: ctx.accounts.mint_auth.to_account_info(),
                account_or_mint: ctx.accounts.share_mint.to_account_info(),
            },
            &[auth_seeds],
        ),
        AuthorityType::MintTokens,
        None,
    )?;

    // The deed side: one Core asset per property, held by the property
    // vault inside its region's collection, its URI naming the property's
    // documents on IPFS. The collection is created up front by the
    // authority, so region branding never rides a developer's transaction.
    require!(
        !ctx.accounts.core_collection.data_is_empty(),
        MarketplaceError::CollectionMissing
    );
    let asset_seeds: &[&[u8]] = &[CORE_ASSET_SEED, &id_bytes, &[ctx.bumps.core_asset]];
    let core_auth_seeds: &[&[u8]] = &[CORE_AUTH_SEED, &[ctx.bumps.core_auth]];
    deed::invoke_core(
        deed::create_asset_data(&name, &uri)?,
        deed::create_asset_metas(
            ctx.accounts.core_asset.key(),
            ctx.accounts.core_collection.key(),
            ctx.accounts.core_auth.key(),
            ctx.accounts.developer.key(),
            ctx.accounts.property_vault.key(),
        ),
        &[
            ctx.accounts.core_asset.to_account_info(),
            ctx.accounts.core_collection.to_account_info(),
            ctx.accounts.core_auth.to_account_info(),
            ctx.accounts.developer.to_account_info(),
            ctx.accounts.property_vault.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.mpl_core_program.to_account_info(),
        ],
        &[asset_seeds, core_auth_seeds],
    )?;

    ctx.accounts.property.core_asset = ctx.accounts.core_asset.key();
    ctx.accounts.property.share_mint = ctx.accounts.share_mint.key();
    ctx.accounts.listing.status = ListingStatus::Listed;

    emit!(PropertyAssetsInitialized {
        listing_id,
        core_asset: ctx.accounts.property.core_asset,
        share_mint: ctx.accounts.property.share_mint,
        share_amount: ctx.accounts.property.share_amount,
    });
    Ok(())
}

/// Create a region's Core collection, the on-chain grouping every property
/// asset in the region mints into. Authority-only, once per region, before
/// the region's first listing reaches `init_property_assets`.
#[derive(Accounts)]
#[instruction(region_id: u16)]
pub struct CreateRegionCollection<'info> {
    #[account(
        mut,
        address = config.authority @ MarketplaceError::NotAuthority,
    )]
    pub authority: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    /// The region, owned by the regions program; its existence is what makes
    /// the id worth a collection.
    #[account(
        seeds = [regions::REGION_SEED, &region_id.to_le_bytes()],
        bump = region.bump,
        seeds::program = regions::ID,
    )]
    pub region: Box<Account<'info, regions::state::Region>>,

    /// CHECK: the region's Core collection PDA; the Core program creates it.
    #[account(mut, seeds = [CORE_COLLECTION_SEED, &region_id.to_le_bytes()], bump)]
    pub core_collection: UncheckedAccount<'info>,

    /// CHECK: update authority of every region collection; a bare PDA.
    #[account(seeds = [CORE_AUTH_SEED], bump)]
    pub core_auth: UncheckedAccount<'info>,

    /// CHECK: the Metaplex Core program, pinned by address.
    #[account(address = MPL_CORE_PROGRAM @ MarketplaceError::WrongProgram)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn create_region_collection_handler(
    ctx: Context<CreateRegionCollection>,
    region_id: u16,
    name: String,
    uri: String,
) -> Result<()> {
    require!(
        !name.is_empty() && name.len() <= MAX_ASSET_NAME_LEN,
        MarketplaceError::InvalidAssetMetadata
    );
    require!(
        uri.len() <= MAX_ASSET_URI_LEN,
        MarketplaceError::InvalidAssetMetadata
    );

    let region_bytes = region_id.to_le_bytes();
    let collection_seeds: &[&[u8]] = &[
        CORE_COLLECTION_SEED,
        &region_bytes,
        &[ctx.bumps.core_collection],
    ];
    deed::invoke_core(
        deed::create_collection_data(&name, &uri)?,
        deed::create_collection_metas(
            ctx.accounts.core_collection.key(),
            ctx.accounts.core_auth.key(),
            ctx.accounts.authority.key(),
        ),
        &[
            ctx.accounts.core_collection.to_account_info(),
            ctx.accounts.core_auth.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.mpl_core_program.to_account_info(),
        ],
        &[collection_seeds],
    )?;

    emit!(RegionCollectionCreated {
        region_id,
        collection: ctx.accounts.core_collection.key(),
    });
    Ok(())
}

#[event]
pub struct RegionCollectionCreated {
    pub region_id: u16,
    pub collection: Pubkey,
}

#[event]
pub struct PropertyAssetsInitialized {
    pub listing_id: u64,
    pub core_asset: Pubkey,
    pub share_mint: Pubkey,
    pub share_amount: u32,
}
