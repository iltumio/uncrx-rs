use std::ops::Range;

pub const CRX_MAGIC_VALUE: [u8; 4] = [0x43, 0x72, 0x32, 0x34];
pub const MAGIC_VALUE_RANGE: Range<usize> = 0..4;
pub const CRX_VERSION_RANGE: Range<usize> = 4..8;
pub const PUBLIC_KEY_LENGTH_RANGE: Range<usize> = 8..12;
pub const SIGNATURE_LENGTH_RANGE: Range<usize> = 12..16;
