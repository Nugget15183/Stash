use std::io::{self, Read};

pub enum Rec {
    Ok { payload: Vec<u8>, end: u64},
    BadCrc {end: u64}, //no trunc but checksum failed
    Torn, //tuncation
}

pub fn read_record<R: Read>(reader: &mut R, pos: u64, file_len: u64) -> io::Result<Rec> {
    let mut header = [0u8; 8];

    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(Rec::Torn),
        Err(e) => return Err(e),
    }

    let len = u32::from_le_bytes(header[0..4].try_into().unwrap()) as u64;
    let crc = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let end = pos + 8 + len;

    if end > file_len {
        return Ok(Rec::Torn);
    }

    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;

    if crc32fast::hash(&payload) != crc {
        return Ok(Rec::BadCrc { end });
    }

    Ok(Rec::Ok { payload, end })
}

#[derive(Default)]
pub struct LoadReport {
    pub bad_ranges: Vec<(u64, u64)>,
    pub truncate_at: Option<u64>,
}