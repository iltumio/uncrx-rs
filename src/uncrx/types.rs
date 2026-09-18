/// Owned CRX contents. Signatures are metadata, NOT verified authentication.
#[derive(Debug)]
pub struct CrxExtension {
    pub version: u32,
    /// CRX2 key, or the first RSA proof (then ECDSA) in a CRX3 header.
    pub public_key: Vec<u8>,
    pub signature: Option<Vec<u8>>,
    pub zip: Vec<u8>,
}

/// Parsed metadata with a borrowed ZIP payload, avoiding an archive-sized copy.
#[derive(Debug)]
pub struct CrxExtensionRef<'a> {
    pub version: u32,
    /// CRX2 key, or the first RSA proof (then ECDSA) in a CRX3 header.
    pub public_key: Vec<u8>,
    pub signature: Option<Vec<u8>>,
    pub zip: &'a [u8],
}
