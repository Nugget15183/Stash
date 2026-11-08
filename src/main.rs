use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind::InvalidData;
use std::io::Seek;
use std::io::{self, BufReader, BufWriter, Read, Write};

//function tags
const PUT: u8 = 0;
const DEL: u8 = 1;
const CLEAR: u8 = 2;

//const variables
//64 MB
const MAX_CHUNK: usize = 64 * 1024 * 1024;

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

fn encode_record(tag: u8, chunks: &[&str]) -> io::Result<Vec<u8>> {
    let mut payload = vec![tag];

    for c in chunks {
        if c.len() > MAX_CHUNK {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Key or Value too large"))
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

//commands

//store struct
struct Store {
    //the hashmap storage itself
    map: HashMap<String, String>,

    //file location for saving and loading from
    save_directory: String,

    //file handle to write updates in real time
    log: BufWriter<File>,
}

impl Store {
    //constructor
    //called using Store::new(...)
    fn new(path: &str) -> io::Result<Self> {
        let file = OpenOptions::new().append(true).create(true).open(path)?;

        let mut store = Store {
            map: HashMap::new(),
            save_directory: path.to_string(),
            log:  BufWriter::new(file),
        };

        store.load()?;
        Ok(store)
    }

    fn apply(&mut self, payload: &[u8]) -> io::Result<()> {
        let (&tag, mut rest) = payload.split_first().ok_or_else(|| invalid("empty payload"))?;

        match tag {
            PUT => {
                let key = take_chunk(&mut rest)?;
                let val = take_chunk(&mut rest)?;
                self.map.insert(key, val);
            }

            DEL => {
                let key = take_chunk(&mut rest)?;
                self.map.remove(&key);
            }

            CLEAR => {
                self.map.clear();
            }

            other => {
                return Err(invalid(format!("Unknown tag {other}")));
            }
        }

        Ok(())
    }

    fn load(&mut self) -> io::Result<()> {
        let file = File::open(&self.save_directory)?;

        let file_len = file.metadata()?.len();

        let mut reader = BufReader::new(file);

        let mut good_end:u64 = 0;
        let mut torn = false;

        while good_end < file_len {
            let mut header = [0u8; 8];

            match reader.read_exact(&mut header) {
                Ok(()) => {},

                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    torn = true; 
                    break;
                }

                Err(e) => return Err(e),
            }

            let len = u32::from_le_bytes(header[0..4].try_into().unwrap()) as u64;
            let crc = u32::from_le_bytes(header[4..8].try_into().unwrap());

            let record_end = good_end + 8 + len;

            if record_end > file_len {
                torn = true;
                break;
            }

            let mut payload = vec![0u8; len as usize];//bound by filesize
            reader.read_exact(&mut payload)?;

            if crc32fast::hash(&payload) != crc {
                if record_end == file_len {
                    torn = true;
                    break;
                }
                return Err(invalid(format!("Checksum mismatch in record at byte {good_end}")));
            }

            self.apply(&payload)?;
            good_end=record_end;
        }

        if torn {
            eprintln!("warning: discarded incomplete record at byte {good_end}");

            OpenOptions::new().write(true).open(&self.save_directory)?.set_len(good_end)?;
        }

        Ok(())
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
        Ok(())
    }


    //enters a value into the db
    fn put(&mut self, key: String, value: String) -> io::Result<()> {
       let rec = encode_record(PUT, &[&key, &value])?;

       self.append(&rec)?;
       self.map.insert(key, value);

       Ok(())
    }

    //returns removed value
    fn del(&mut self, key: &str) -> io::Result<Option<String>> {
        if !self.map.contains_key(key) {
            return Ok(None);
        }

        let rec = encode_record(DEL, &[key])?;
        self.append(&rec)?;

        Ok(self.map.remove(key))
    }

    //clears the map
    fn clear(&mut self) -> io::Result<()> {
        let rec = encode_record(CLEAR, &[])?;
        self.append(&rec)?;
        self.map.clear();
        Ok(())
    }
}

fn main() {
    let mut store = match Store::new("save.bin") {
        Ok(s) => s,

        Err(e) => {
            eprintln!("Failed to open store: {e}");
            std::process::exit(1);
        },
    };

    io::stdout().flush().expect("Failed to flush stdout");

    let mut conf_clear = false;

    loop {
        print!("> ");
        io::stdout().flush().expect("Failed to flush stdout");

        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input).expect("Failed to read line");

        if bytes==0 {
            break;
        }

        let args: Vec<&str> = input.split_whitespace().collect();

        //replacement for if/else chains
        match args.as_slice() {
            ["put", key, value] => {
                store.put(key.to_string(), value.to_string()).expect("Failed to write key to store");
                println!("Set key {key} to value {value} successfully");
            }

            ["get", key] => match store.get(&key.to_string()) {
                Some(v) => {
                    println!("Value at key {key}: {v}")
                }
                None => {
                    println!("Key {key} not found.")
                }
            }

            ["del", key] => match store.del(key).expect("Failed to write deletion of key") {
                Some(_) => {
                    println!("Key {key} deleted successfully")
                }

                None => {
                    println!("Key {key} not found")
                }
            } 

            ["list"] => {
                for key in store.map.keys() {
                    println!("{}", key)
                }
            }

            ["exit"] => {
                break;
            }

            ["clear"] => {
                if !conf_clear {
                    println!("Are you sure you want to clear all keys? Retype 'clear' to confirm");
                    conf_clear=true;
                } else {
                    match store.clear() {
                        Ok(_) => {
                            println!("Cleared all keys in the store successfully");
                        },
                        Err(_) => {
                            println!("Failed to clear store.");
                        },
                    }
                    conf_clear=false;
                }
            }

            [] => {}//empty line
            _ => println!("Unknown commmand")
        }
    }

}
