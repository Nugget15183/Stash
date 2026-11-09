//function tags
pub const PUT:u8 = 0;
pub const DEL:u8 = 1;
pub const CLEAR:u8 = 2;

//64MB
pub const MAX_CHUNK:usize = 64 * 1024 * 1024;

pub const COMPACT_MIN_RECORDS: u64 = 1000;