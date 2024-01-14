#[derive(Debug)]
pub struct CrxExtension {
    pub version: u32,
    pub public_key: Vec<u8>,
    pub signature: Option<Vec<u8>>,
    pub zip: Vec<u8>,
}
