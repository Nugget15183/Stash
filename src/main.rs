use chrono::Utc;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Write};

use crate::load::LoadReport;

mod consts;
mod examples;
mod load;

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

fn encode_record(tag: u8, chunks: &[&str]) -> io::Result<Vec<u8>> {
    let mut payload = vec![tag];

    for c in chunks {
        if c.len() > consts::MAX_CHUNK {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Key or Value too large",
            ));
        }

        payload.extend_from_slice(&(c.len() as u32).to_le_bytes());
        payload.extend_from_slice(c.as_bytes());
    }

    let mut rec = Vec::with_capacity(8 + payload.len());
    rec.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    rec.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    rec.extend_from_slice(&payload);
    Ok(rec)
}

fn take_chunk(buf: &mut &[u8]) -> io::Result<String> {
    if buf.len() < 4 {
        return Err(invalid("payload too short for chunk length"));
    }

    let len = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
    let rest = &buf[4..];

    if rest.len() < len {
        return Err(invalid("chunk extends past end of payload"));
    }

    let (bytes, tail) = rest.split_at(len);

    *buf = tail;

    String::from_utf8(bytes.to_vec()).map_err(|e| invalid(e.to_string()))
}

//store struct
struct Store {
    //the hashmap storage itself
    map: HashMap<String, String>,

    //file location for saving and loading from
    save_directory: String,

    //file handle to write updates in real time
    log: BufWriter<File>,

    record_count: u64
}

impl Store {
    fn apply(&mut self, payload: &[u8]) -> io::Result<()> {
        let (&tag, mut rest) = payload
            .split_first()
            .ok_or_else(|| invalid("empty payload"))?;

        match tag {
            consts::PUT => {
                let key = take_chunk(&mut rest)?;
                let val = take_chunk(&mut rest)?;
                self.map.insert(key, val);
            }

            consts::DEL => {
                let key = take_chunk(&mut rest)?;
                self.map.remove(&key);
            }

            consts::CLEAR => self.map.clear(),

            other => return Err(invalid(format!("Unknown tag {other}"))),
        }

        Ok(())
    }

    fn remove_bad(&mut self, bad_start: u64, bad_end: u64) -> io::Result<()> {
        self.log.flush()?;

        let buf = fs::read(&self.save_directory)?;

        let blen: u64 = buf.len() as u64;

        if bad_start > bad_end || bad_end > blen as u64 {
            return Err(invalid(format!(
                "Bad range {bad_start}..{bad_end} is invalid for the file of {} bytes",
                blen
            )));
        }

        let (start, end) = (bad_start as usize, bad_end as usize);

        let temp_dir = format!("{}.tmp", &self.save_directory);

        let mut out = File::create(&temp_dir)?;
        out.write_all(&buf[..start])?;
        out.write_all(&buf[end..])?;
        out.sync_all()?;

        fs::rename(&temp_dir, &self.save_directory)?;

        let file = OpenOptions::new().append(true).open(&self.save_directory)?;
        self.log = BufWriter::new(file);

        Ok(())
    }

    fn load(&mut self, repair: bool) -> io::Result<load::LoadReport> {
        let file = File::open(&self.save_directory)?;
        let file_len = file.metadata()?.len();
        let mut reader = BufReader::new(file);

        let mut report = load::LoadReport::default();
        let mut pos: u64 = 0;
        let mut pending: Option<(u64, u64)> = None;

        while pos < file_len {
            match load::read_record(&mut reader, pos, file_len)? {
                load::Rec::Ok { payload, end } => {
                    self.apply(&payload)?;

                    if let Some(range) = pending.take() {
                        report.bad_ranges.push(range); //last len was verified
                    }
                    pos = end;
                    self.record_count += 1;
                }

                load::Rec::BadCrc { end } => {
                    if let Some((start, _)) = pending.take() {
                        //2 bad recs in a row means cut everything after, cant verify length was preserved
                        report.truncate_at = Some(start);
                        break;
                    }

                    if end == file_len {
                        //damaged final rec
                        report.truncate_at = Some(pos);
                        break;
                    }

                    if !repair {
                        return Err(invalid(format!(
                            "checksum mismatched in record at byte {pos}"
                        )));
                    }

                    pending = Some((pos, end));
                    pos = end;
                }

                load::Rec::Torn {} => {
                    //bad rec was pending its len is wrong so truncate it
                    report.truncate_at = Some(pending.take().map_or(pos, |(start, _)| start));
                    break;
                }
            }
        }

        Ok(report)
    }

    fn open(path: &str, repair: bool) -> io::Result<(Self, load::LoadReport)> {
        let _ = fs::remove_file(format!("{path}.compact.tmp"));

        let file = OpenOptions::new().append(true).create(true).open(path)?;

        if repair {
            let stamp = Utc::now().format("%Y%m%d-%H%M%S");
            let fpath = format!("{path}.{stamp}.bak");

            fs::copy(path, fpath)?;
        }

        let mut store = Store {
            map: HashMap::new(),
            save_directory: path.to_string(),
            log: BufWriter::new(file),
            record_count: 0,
        };

        let report = store.load(repair)?;

        if let Some(t) = report.truncate_at {
            eprintln!("warning: discarded everything from byte {t}");
            OpenOptions::new().write(true).open(path)?.set_len(t)?;
        }

        // last to first, so earlier offsets stay valid
        for &(start, end) in report.bad_ranges.iter().rev() {
            store.remove_bad(start, end)?;
        }

        Ok((store, report))
    }

    fn compact(&mut self) -> io::Result<()> {
        self.log.flush()?;

        let temp_path = format!("{}.compact.tmp", self.save_directory);

        {
            let mut out = BufWriter::new(File::create(&temp_path)?);

            for (k, v) in &self.map {
                let rec = encode_record(consts::PUT, &[k.as_str(), v.as_str()])?;
                out.write(&rec)?;
            }

            fs::rename(&temp_path, &self.save_directory)?;

            let file = OpenOptions::new().append(true).open(&self.save_directory)?;
            self.log = BufWriter::new(file);
            self.record_count = self.map.len() as u64;

            Ok(())
        }
    }

    fn maybe_compact(&mut self) -> io::Result<()> {
        let live = self.map.len() as u64;

        if self.record_count > consts::COMPACT_MIN_RECORDS && self.record_count > 2 * live {
            self.compact()?;
        }

        Ok(())
    }

    //constructor
    //called using Store::new(...)
    fn new(path: &str) -> io::Result<Self> {
        Ok(Self::open(path, false)?.0)
    }

    fn open_repair(path: &str) -> io::Result<(Self, LoadReport)> {
        Self::open(path, true)
    }

    //read only
    //param format is (name: type, name: type)
    fn get(&self, key: &str) -> Option<&String> {
        self.map.get(key)
    }

    fn append(&mut self, rec: &[u8]) -> io::Result<()> {
        self.log.write_all(rec)?;
        self.log.flush()?;
        // self.log.get_ref().sync_data()?; //for extra durability, but slow
        self.record_count += 1;
        Ok(())
    }

    //enters a value into the db
    fn put(&mut self, key: String, value: String) -> io::Result<()> {
        let rec = encode_record(consts::PUT, &[&key, &value])?;

        self.append(&rec)?;
        self.map.insert(key, value);

        Ok(())
    }

    //returns removed value
    fn del(&mut self, key: &str) -> io::Result<Option<String>> {
        if !self.map.contains_key(key) {
            return Ok(None);
        }

        let rec = encode_record(consts::DEL, &[key])?;
        self.append(&rec)?;

        Ok(self.map.remove(key))
    }

    //clears the map
    fn clear(&mut self) -> io::Result<()> {
        let rec = encode_record(consts::CLEAR, &[])?;
        self.append(&rec)?;
        self.map.clear();
        Ok(())
    }
}

fn main() {
    examples::terminal();
}
