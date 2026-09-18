#![allow(dead_code)]
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

pub fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        if name.ends_with('/') {
            writer.add_directory(*name, options).unwrap();
        } else {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
    }
    writer.finish().unwrap().into_inner()
}

pub fn crx2(key: &[u8], signature: &[u8], zip: &[u8]) -> Vec<u8> {
    let mut data = b"Cr24".to_vec();
    for n in [2, key.len() as u32, signature.len() as u32] {
        data.extend(n.to_le_bytes());
    }
    data.extend(key);
    data.extend(signature);
    data.extend(zip);
    data
}

pub fn crx3(header: &[u8], zip: &[u8]) -> Vec<u8> {
    let mut data = b"Cr24".to_vec();
    data.extend(3u32.to_le_bytes());
    data.extend((header.len() as u32).to_le_bytes());
    data.extend(header);
    data.extend(zip);
    data
}
