use std::ops::Range;

use anyhow::{bail, ensure, Context, Result};
use prost::Message;

use super::{
    constants::*,
    types::{CrxExtension, CrxExtensionRef},
};

/// Maximum combined key/signature or Protobuf header size (16 MiB).
pub const MAX_HEADER_BYTES: usize = 16 * 1024 * 1024;

pub fn get_slice_from_range(data: &[u8], range: Range<usize>) -> Result<&[u8]> {
    data.get(range).context("Invalid or truncated CRX range")
}

fn read_u32(data: &[u8], range: Range<usize>) -> Result<u32> {
    let bytes: [u8; 4] = get_slice_from_range(data, range)?.try_into()?;
    Ok(u32::from_le_bytes(bytes))
}

pub fn get_crx_header(data: &[u8]) -> Result<[u8; 4]> {
    Ok(get_slice_from_range(data, MAGIC_VALUE_RANGE)?.try_into()?)
}

pub fn get_crx_version(data: &[u8]) -> Result<u32> {
    read_u32(data, CRX_VERSION_RANGE)
}

pub fn is_valid_crx(magic: &[u8; 4]) -> Result<bool> {
    Ok(magic == &CRX_MAGIC_VALUE)
}

/// CRX2 key length; for CRX3 these bytes describe the entire Protobuf header.
pub fn get_public_key_length(data: &[u8]) -> Result<u32> {
    read_u32(data, PUBLIC_KEY_LENGTH_RANGE)
}

pub fn get_signature_key_length(data: &[u8]) -> Result<u32> {
    read_u32(data, SIGNATURE_LENGTH_RANGE)
}

// Wire layout from Chromium components/crx_file/crx3.proto. Unknown fields are
// skipped by prost; no custom recursive Protobuf parser is used.
#[derive(Message)]
struct Crx3Header {
    #[prost(message, repeated, tag = "2")]
    rsa: Vec<Proof>,
    #[prost(message, repeated, tag = "3")]
    ecdsa: Vec<Proof>,
}

#[derive(Message)]
struct Proof {
    #[prost(bytes = "vec", tag = "1")]
    public_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    signature: Vec<u8>,
}

fn checked_end(start: usize, length: usize) -> Result<usize> {
    start.checked_add(length).context("CRX length overflow")
}

/// Parse CRX2 or CRX3; this does not verify signatures or validate ZIP contents.
/// Unknown versions, malformed headers and headers larger than 16 MiB fail.
/// CRX3 exposes the first RSA proof, or the first ECDSA proof if no RSA exists.
pub fn parse_crx_ref(data: &[u8]) -> Result<CrxExtensionRef<'_>> {
    ensure!(
        get_crx_header(data)? == CRX_MAGIC_VALUE,
        "Invalid CRX magic"
    );
    let version = get_crx_version(data)?;
    let (public_key, signature, offset) = match version {
        2 => {
            let key_len = get_public_key_length(data)? as usize;
            let sig_len = get_signature_key_length(data)? as usize;
            let total = checked_end(key_len, sig_len)?;
            ensure!(total <= MAX_HEADER_BYTES, "CRX header exceeds limit");
            let key_end = checked_end(16, key_len)?;
            let end = checked_end(key_end, sig_len)?;
            let key = get_slice_from_range(data, 16..key_end)?.to_vec();
            let sig = get_slice_from_range(data, key_end..end)?.to_vec();
            (key, (!sig.is_empty()).then_some(sig), end)
        }
        3 => {
            let length = get_public_key_length(data)? as usize;
            ensure!(length <= MAX_HEADER_BYTES, "CRX header exceeds limit");
            let end = checked_end(12, length)?;
            let header = Crx3Header::decode(get_slice_from_range(data, 12..end)?)
                .context("Invalid CRX3 Protobuf header")?;
            let proof = header.rsa.into_iter().chain(header.ecdsa).next();
            let (key, sig) = proof
                .map(|p| (p.public_key, p.signature))
                .unwrap_or_default();
            (key, (!sig.is_empty()).then_some(sig), end)
        }
        _ => bail!("Unsupported CRX version: {version}"),
    };
    let zip = data.get(offset..).context("Truncated CRX payload")?;
    ensure!(!zip.is_empty(), "Missing ZIP payload");
    Ok(CrxExtensionRef {
        version,
        public_key,
        signature,
        zip,
    })
}

/// Compatibility API returning owned ZIP bytes. Prefer `parse_crx_ref` when the
/// input buffer can outlive the parsed result.
pub fn parse_crx(data: &[u8]) -> Result<CrxExtension> {
    let parsed = parse_crx_ref(data)?;
    Ok(CrxExtension {
        version: parsed.version,
        public_key: parsed.public_key,
        signature: parsed.signature,
        zip: parsed.zip.to_vec(),
    })
}
