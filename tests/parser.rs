mod common;
use uncrx_rs::uncrx::helpers::{get_slice_from_range, parse_crx, parse_crx_ref};

#[test]
fn crx2_signature_follows_key_and_zip_is_borrowed() {
    let data = common::crx2(b"KEY", b"SIGNATURE", b"ZIP");
    let parsed = parse_crx_ref(&data).unwrap();
    assert_eq!(parsed.public_key, b"KEY");
    assert_eq!(parsed.signature.as_deref(), Some(b"SIGNATURE".as_slice()));
    assert_eq!(parsed.zip, b"ZIP");
    assert_eq!(parsed.zip.as_ptr(), data[data.len() - 3..].as_ptr());
    assert_eq!(parse_crx(&data).unwrap().zip, parsed.zip);
}

#[test]
fn crx3_decodes_rsa_and_ecdsa_proofs() {
    // Independent wire fixture: field 2 or 3, nested key field 1 and signature field 2.
    for tag in [0x12, 0x1a] {
        let header = [
            tag, 10, 0x0a, 3, b'K', b'E', b'Y', 0x12, 3, b'S', b'I', b'G',
        ];
        let data = common::crx3(&header, b"ZIP");
        let parsed = parse_crx_ref(&data).unwrap();
        assert_eq!(parsed.public_key, b"KEY");
        assert_eq!(parsed.signature.as_deref(), Some(b"SIG".as_slice()));
        assert_eq!(parsed.zip, b"ZIP");
    }
}

#[test]
fn rejects_unknown_versions_truncation_and_oversized_headers() {
    let valid = common::crx2(b"KEY", b"SIGNATURE", b"ZIP");
    for len in 0..=28 {
        assert!(parse_crx(&valid[..len]).is_err(), "prefix {len}");
    }
    for version in [0u32, 1, 4, 99, u32::MAX] {
        let mut data = valid.clone();
        data[4..8].copy_from_slice(&version.to_le_bytes());
        assert!(parse_crx(&data).is_err());
    }
    for offset in [8, 12] {
        let mut data = valid.clone();
        data[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_crx(&data).is_err());
    }
    assert!(parse_crx(&common::crx3(&[0x12, 255], b"ZIP")).is_err());
    let reversed = std::ops::Range { start: 3, end: 1 };
    assert!(get_slice_from_range(b"1234", reversed).is_err());
}

#[test]
fn arbitrary_small_inputs_do_not_panic() {
    let mut state = 123456789u32;
    for len in 0..256 {
        let mut data = vec![0; len];
        for byte in &mut data {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = state as u8;
        }
        for version in [2u32, 3, 99] {
            if len >= 8 {
                data[..4].copy_from_slice(b"Cr24");
                data[4..8].copy_from_slice(&version.to_le_bytes());
            }
            let _ = parse_crx(&data);
        }
    }
}

#[test]
fn real_extension_fixture_parses() {
    let parsed = parse_crx_ref(include_bytes!("../src/mock/test-extension.crx")).unwrap();
    assert!(!parsed.zip.is_empty());
    assert!(!parsed.public_key.is_empty());
    assert!(parsed.signature.is_some());
}
