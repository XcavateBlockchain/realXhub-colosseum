//! Hand-built CPIs into Metaplex Core for the property deeds. The mpl-core
//! crate stays out of the on-chain build on purpose: its plugin
//! deserializers blow the SBF stack-frame limit, and we only ever create
//! and burn plain assets. The tests compare these encodings byte-for-byte
//! against the crate's own builders, so drift fails loudly.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;

use crate::constants::MPL_CORE_PROGRAM;

/// Core marks an absent optional account with its own program id.
const NONE: Pubkey = MPL_CORE_PROGRAM;

pub fn create_collection_data(name: &str, uri: &str) -> Result<Vec<u8>> {
    // CreateCollectionV2: name, uri, no plugins, no adapters.
    let mut data = vec![21u8];
    name.serialize(&mut data)?;
    uri.serialize(&mut data)?;
    data.extend_from_slice(&[0, 0]);
    Ok(data)
}

pub fn create_collection_metas(
    collection: Pubkey,
    update_authority: Pubkey,
    payer: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(collection, true),
        AccountMeta::new_readonly(update_authority, false),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
    ]
}

pub fn create_asset_data(name: &str, uri: &str) -> Result<Vec<u8>> {
    // CreateV2: account data state, name, uri, no plugins, no adapters.
    let mut data = vec![20u8, 0];
    name.serialize(&mut data)?;
    uri.serialize(&mut data)?;
    data.extend_from_slice(&[0, 0]);
    Ok(data)
}

pub fn create_asset_metas(
    asset: Pubkey,
    collection: Pubkey,
    authority: Pubkey,
    payer: Pubkey,
    owner: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(asset, true),
        AccountMeta::new(collection, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new_readonly(NONE, false),
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        AccountMeta::new_readonly(NONE, false),
    ]
}

pub fn burn_asset_data() -> Vec<u8> {
    // BurnV1: no compression proof.
    vec![12, 0]
}

pub fn burn_asset_metas(
    asset: Pubkey,
    collection: Pubkey,
    payer: Pubkey,
    owner: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(asset, false),
        AccountMeta::new(collection, false),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(owner, true),
        AccountMeta::new_readonly(NONE, false),
        AccountMeta::new_readonly(NONE, false),
    ]
}

pub(crate) fn invoke_core<'info>(
    data: Vec<u8>,
    metas: Vec<AccountMeta>,
    infos: &[AccountInfo<'info>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let ix = Instruction {
        program_id: MPL_CORE_PROGRAM,
        accounts: metas,
        data,
    };
    invoke_signed(&ix, infos, signer_seeds).map_err(Into::into)
}
