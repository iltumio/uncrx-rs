use std::ops::Range;

use super::{
    constants::{
        CRX_MAGIC_VALUE, CRX_VERSION_RANGE, MAGIC_VALUE_RANGE, PUBLIC_KEY_LENGTH_RANGE,
        SIGNATURE_LENGTH_RANGE,
    },
    types::CrxExtension,
};

pub fn get_crx_header(data: &Vec<u8>) -> anyhow::Result<[u8; 4]> {
    let slice = get_slice_from_range(data, MAGIC_VALUE_RANGE)?;

    let mut magic = [0u8; 4];
    magic.copy_from_slice(slice);

    Ok(magic)
}

pub fn get_crx_version(data: &Vec<u8>) -> anyhow::Result<u32> {
    let slice = get_slice_from_range(data, CRX_VERSION_RANGE)?;

    let mut version = [0u8; 4];
    version.copy_from_slice(slice);

    Ok(u32::from_le_bytes(version))
}

pub fn is_valid_crx(magic: &[u8; 4]) -> anyhow::Result<bool> {
    Ok(magic == &CRX_MAGIC_VALUE)
}

pub fn get_public_key_length(data: &Vec<u8>) -> anyhow::Result<u32> {
    let slice = get_slice_from_range(data, PUBLIC_KEY_LENGTH_RANGE)?;

    let mut public_key_length = [0u8; 4];
    public_key_length.copy_from_slice(slice);

    Ok(u32::from_le_bytes(public_key_length))
}

pub fn get_signature_key_length(data: &Vec<u8>) -> anyhow::Result<u32> {
    let slice = get_slice_from_range(data, SIGNATURE_LENGTH_RANGE)?;

    let mut signature_length = [0u8; 4];
    signature_length.copy_from_slice(slice);

    Ok(u32::from_le_bytes(signature_length))
}

pub fn get_slice_from_range(data: &Vec<u8>, range: Range<usize>) -> anyhow::Result<&[u8]> {
    if data.len() < range.end {
        return Err(anyhow::anyhow!("Data is too short"));
    }

    Ok(&data[range])
}

pub fn parse_crx(data: &Vec<u8>) -> anyhow::Result<CrxExtension> {
    let header = get_crx_header(data)?;
    let is_valid = is_valid_crx(&header)?;

    if !is_valid {
        return Err(anyhow::anyhow!("Invalid CRX file"));
    }

    let version = get_crx_version(data)?;

    let public_key_length = get_public_key_length(data)?;

    let public_key = data[16..(16 + public_key_length as usize)].to_vec();

    let signature_key_length = if version <= 2 {
        get_signature_key_length(data)?
    } else {
        0
    };

    let signature = match signature_key_length {
        0 => None,
        _ => Some(data[16..(16 + signature_key_length as usize)].to_vec()),
    };

    let header = if version <= 2 { 16 } else { 12 };

    let zip_start_offset = (header + signature_key_length + public_key_length) as usize;

    let zip = data[zip_start_offset..].to_vec();

    let extension = CrxExtension {
        version,
        public_key,
        signature,
        zip,
    };

    Ok(extension)
}
