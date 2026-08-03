use anchor_lang::prelude::*;

use crate::constants::{
    CONFIG_SEED, LAWYER_CANDIDATE_SEED, LAWYER_SEED, LISTING_SEED, PROPERTY_SEED,
};
use crate::error::MarketplaceError;
use crate::state::{
    Config, DocumentStatus, Lawyer, LawyerCandidacy, Listing, ListingStatus, PropertyAsset,
    MAX_SPV_CANDIDATES,
};

use xcavate_whitelist::state::{Role, RoleAccount};

/// The gates every lawyer engagement shares: the sale is sold out, the legal
/// process still has time, and the lawyer serves the property's region.
fn check_engagement_open(
    listing: &Listing,
    property: &PropertyAsset,
    registry: &Lawyer,
) -> Result<()> {
    require!(
        listing.status == ListingStatus::SoldOut,
        MarketplaceError::ListingNotActive
    );
    require!(
        Clock::get()?.unix_timestamp <= listing.legal_deadline,
        MarketplaceError::LegalProcessExpired
    );
    require!(
        registry.region_id == property.region_id,
        MarketplaceError::WrongRegion
    );
    Ok(())
}

/// The developer engages their own lawyer directly by address. The named
/// wallet must hold a compliant Lawyer role and a registry entry in the
/// property's region; costs come out of the shared fee pot, so they are
/// capped by what the SPV side hasn't committed. Developer-role only, and
/// only the listing's own developer.
#[derive(Accounts)]
#[instruction(listing_id: u64, lawyer: Pubkey)]
pub struct AssignDeveloperLawyer<'info> {
    pub developer: Signer<'info>,

    /// The caller's RealEstateDeveloper role, owned by the roles program.
    #[account(
        seeds = [
            xcavate_whitelist::ROLE_SEED,
            developer.key().as_ref(),
            &[Role::RealEstateDeveloper.seed_byte()],
        ],
        bump = developer_role.bump,
        seeds::program = xcavate_whitelist::ID,
    )]
    pub developer_role: Box<Account<'info, RoleAccount>>,

    /// The named lawyer's role; they carry the legal responsibility, so the
    /// compliance flag is checked even though they aren't the signer.
    #[account(
        seeds = [
            xcavate_whitelist::ROLE_SEED,
            lawyer.as_ref(),
            &[Role::Lawyer.seed_byte()],
        ],
        bump = lawyer_role.bump,
        seeds::program = xcavate_whitelist::ID,
        constraint = lawyer_role.is_compliant() @ MarketplaceError::NotCompliant,
    )]
    pub lawyer_role: Box<Account<'info, RoleAccount>>,

    /// The named lawyer's registry entry; takes the case.
    #[account(
        mut,
        seeds = [LAWYER_SEED, lawyer.as_ref()],
        bump = registry.bump,
    )]
    pub registry: Box<Account<'info, Lawyer>>,

    #[account(
        mut,
        seeds = [LISTING_SEED, &listing_id.to_le_bytes()],
        bump = listing.bump,
        constraint = listing.developer == developer.key() @ MarketplaceError::NotListingDeveloper,
    )]
    pub listing: Box<Account<'info, Listing>>,

    #[account(
        seeds = [PROPERTY_SEED, &listing_id.to_le_bytes()],
        bump = property.bump,
    )]
    pub property: Box<Account<'info, PropertyAsset>>,
}

pub fn assign_developer_lawyer_handler(
    ctx: Context<AssignDeveloperLawyer>,
    listing_id: u64,
    lawyer: Pubkey,
    costs: u64,
) -> Result<()> {
    let listing = &ctx.accounts.listing;
    check_engagement_open(listing, &ctx.accounts.property, &ctx.accounts.registry)?;
    require!(
        listing.developer_lawyer.lawyer == Pubkey::default(),
        MarketplaceError::LawyerJobTaken
    );
    // One lawyer never acts for both sides.
    require!(
        lawyer != listing.spv_lawyer.lawyer,
        MarketplaceError::ConflictOfInterest
    );
    // Both sides are paid from the same fee pot, so the check is against
    // what the other side has already committed, not the whole pot.
    require!(
        costs.saturating_add(listing.spv_lawyer.costs) <= listing.total_fee_quote()?,
        MarketplaceError::CostsExceedFees
    );

    ctx.accounts.registry.active_cases = ctx
        .accounts
        .registry
        .active_cases
        .checked_add(1)
        .ok_or(MarketplaceError::Overflow)?;
    let listing = &mut ctx.accounts.listing;
    listing.developer_lawyer.lawyer = lawyer;
    listing.developer_lawyer.costs = costs;
    listing.developer_lawyer.doc_status = DocumentStatus::Pending;

    emit!(DeveloperLawyerAssigned {
        listing_id,
        lawyer,
        costs,
    });
    Ok(())
}

/// A registered lawyer stands for election as the SPV's lawyer, naming their
/// costs. The first candidacy opens the voting window; later ones join the
/// same round while it runs. The sponsor fronts the candidacy's rent.
/// Lawyer-role only and compliance-gated: engaging on a live sale is where
/// legal responsibility starts.
#[derive(Accounts)]
#[instruction(listing_id: u64, round: u64)]
pub struct ClaimSpvCase<'info> {
    pub lawyer: Signer<'info>,

    /// The sponsor wallet fronting the candidacy's rent.
    #[account(mut, address = config.rent_collector @ MarketplaceError::NotRentCollector)]
    pub payer: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    /// The caller's Lawyer role, owned by the roles program.
    #[account(
        seeds = [
            xcavate_whitelist::ROLE_SEED,
            lawyer.key().as_ref(),
            &[Role::Lawyer.seed_byte()],
        ],
        bump = lawyer_role.bump,
        seeds::program = xcavate_whitelist::ID,
        constraint = lawyer_role.is_compliant() @ MarketplaceError::NotCompliant,
    )]
    pub lawyer_role: Box<Account<'info, RoleAccount>>,

    /// The caller's registry entry; proves registration and carries the region.
    #[account(
        seeds = [LAWYER_SEED, lawyer.key().as_ref()],
        bump = registry.bump,
    )]
    pub registry: Box<Account<'info, Lawyer>>,

    #[account(
        mut,
        seeds = [LISTING_SEED, &listing_id.to_le_bytes()],
        bump = listing.bump,
    )]
    pub listing: Box<Account<'info, Listing>>,

    #[account(
        seeds = [PROPERTY_SEED, &listing_id.to_le_bytes()],
        bump = property.bump,
    )]
    pub property: Box<Account<'info, PropertyAsset>>,

    /// The candidacy, one per lawyer per round; `init` rejects standing twice.
    #[account(
        init,
        payer = payer,
        space = 8 + LawyerCandidacy::INIT_SPACE,
        seeds = [
            LAWYER_CANDIDATE_SEED,
            &listing_id.to_le_bytes(),
            &round.to_le_bytes(),
            lawyer.key().as_ref(),
        ],
        bump,
    )]
    pub candidacy: Box<Account<'info, LawyerCandidacy>>,

    pub system_program: Program<'info, System>,
}

pub fn claim_spv_case_handler(
    ctx: Context<ClaimSpvCase>,
    listing_id: u64,
    round: u64,
    costs: u64,
) -> Result<()> {
    let listing = &ctx.accounts.listing;
    check_engagement_open(listing, &ctx.accounts.property, &ctx.accounts.registry)?;
    require!(
        ctx.accounts.property.spv_created,
        MarketplaceError::SpvNotCreated
    );
    require!(
        listing.spv_lawyer.lawyer == Pubkey::default(),
        MarketplaceError::LawyerJobTaken
    );
    let lawyer = ctx.accounts.lawyer.key();
    require!(
        lawyer != listing.developer_lawyer.lawyer,
        MarketplaceError::ConflictOfInterest
    );
    require!(
        costs.saturating_add(listing.developer_lawyer.costs) <= listing.total_fee_quote()?,
        MarketplaceError::CostsExceedFees
    );

    let now = Clock::get()?.unix_timestamp;
    let listing = &mut ctx.accounts.listing;
    let voting_time = listing.lawyer_voting_time;
    let deadline = listing.legal_deadline;
    let election = &mut listing.spv_election;
    if election.expiry == 0 {
        // First candidacy: a new round opens and the clock starts. The
        // window never outlives the legal deadline, past which the winner
        // could not be assigned anyway.
        require!(
            round == election.round + 1,
            MarketplaceError::WrongElectionRound
        );
        election.round = round;
        election.expiry = now
            .checked_add(voting_time)
            .ok_or(MarketplaceError::Overflow)?
            .min(deadline);
        election.candidate_count = 0;
    } else {
        require!(
            round == election.round,
            MarketplaceError::WrongElectionRound
        );
        require!(now < election.expiry, MarketplaceError::VotingClosed);
    }
    election.candidate_count = election
        .candidate_count
        .checked_add(1)
        .ok_or(MarketplaceError::Overflow)?;
    require!(
        election.candidate_count <= MAX_SPV_CANDIDATES,
        MarketplaceError::TooManyCandidates
    );

    let candidacy = &mut ctx.accounts.candidacy;
    candidacy.listing_id = listing_id;
    candidacy.round = round;
    candidacy.lawyer = lawyer;
    candidacy.costs = costs;
    candidacy.vote_power = 0;
    candidacy.bump = ctx.bumps.candidacy;

    emit!(SpvCaseClaimed {
        listing_id,
        round,
        lawyer,
        costs,
        expiry: listing.spv_election.expiry,
    });
    Ok(())
}

/// A lawyer steps back from a case they were engaged on, reopening that side,
/// as long as they haven't confirmed documents yet. Not role-gated: stepping
/// back is an exit, and the registry PDA plus the assignment on the listing
/// prove who the caller is.
#[derive(Accounts)]
#[instruction(listing_id: u64)]
pub struct ResignFromCase<'info> {
    pub lawyer: Signer<'info>,

    #[account(
        mut,
        seeds = [LAWYER_SEED, lawyer.key().as_ref()],
        bump = registry.bump,
    )]
    pub registry: Box<Account<'info, Lawyer>>,

    #[account(
        mut,
        seeds = [LISTING_SEED, &listing_id.to_le_bytes()],
        bump = listing.bump,
    )]
    pub listing: Box<Account<'info, Listing>>,
}

pub fn resign_from_case_handler(ctx: Context<ResignFromCase>, listing_id: u64) -> Result<()> {
    let lawyer = ctx.accounts.lawyer.key();
    let listing = &mut ctx.accounts.listing;
    let side = if listing.developer_lawyer.lawyer == lawyer {
        &mut listing.developer_lawyer
    } else if listing.spv_lawyer.lawyer == lawyer {
        &mut listing.spv_lawyer
    } else {
        return err!(MarketplaceError::NotCaseLawyer);
    };
    // Once documents are confirmed the sale is relying on this lawyer.
    require!(
        side.doc_status == DocumentStatus::Pending,
        MarketplaceError::AlreadyConfirmed
    );
    side.lawyer = Pubkey::default();
    side.costs = 0;

    ctx.accounts.registry.active_cases = ctx
        .accounts
        .registry
        .active_cases
        .checked_sub(1)
        .ok_or(MarketplaceError::Overflow)?;

    emit!(LawyerResigned { listing_id, lawyer });
    Ok(())
}

#[event]
pub struct DeveloperLawyerAssigned {
    pub listing_id: u64,
    pub lawyer: Pubkey,
    pub costs: u64,
}

#[event]
pub struct SpvCaseClaimed {
    pub listing_id: u64,
    pub round: u64,
    pub lawyer: Pubkey,
    pub costs: u64,
    pub expiry: i64,
}

#[event]
pub struct LawyerResigned {
    pub listing_id: u64,
    pub lawyer: Pubkey,
}
