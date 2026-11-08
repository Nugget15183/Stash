use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Seek;
use std::io::{self, BufReader, BufWriter, Read, Write};

//function tags
const PUT: u8 = 0;
const DEL: u8 = 1;
const CLEAR: u8 = 2;

//const variables
//64 MB
const MAX_READ_WRITE: u32 = 64 * 1024 * 1024;

fn write_chunk<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let len = (s.len() as u32).min(MAX_READ_WRITE);
 
    w.write_all(&(len).to_le_bytes())?;
    w.write_all(s.as_bytes())
}

fn read_chunk<R: Read + Seek>(reader: &mut R, file_len: u64) -> io::Result<Option<String>> {
    let mut len_buf = [0u8; 4];

    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let len = u32::from_le_bytes(len_buf) as usize;
    let pos = reader.stream_position()?;

    if len as u64 > file_len.saturating_sub(pos) {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Record extends past end of file"));
    }

    if len > MAX_READ_WRITE.try_into().unwrap() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("chunk length {len} exceeds limit of {MAX_READ_WRITE}!")
        ))
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;

    String::from_utf8(buf).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn read_required<R: Read + Seek>(reader: &mut R, file_len: u64) -> io::Result<String> {
    read_chunk(reader, file_len)?.ok_or_else(|| {
        io::Error::new(io::ErrorKind::UnexpectedEof, "Trunctuated record")
    })
}

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

    fn load(&mut self) -> io::Result<()> {
        let file = File::open(&self.save_directory)?;
        let file_len = file.metadata()?.len();
        let mut reader = BufReader::new(file);

        loop {
            let record_start = reader.stream_position()?;

            let mut tag = [0u8; 1];

            match reader.read_exact(&mut tag) {
                Ok(()) => {}

                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }

            let result: io::Result<()> = (|| {
            match tag[0] {
                    PUT => {
                        let key = read_required(&mut reader, file_len)?;
                        let val = read_required(&mut reader, file_len)?;
                        self.map.insert(key, val);
                    }

                    DEL => {
                        let key = read_required(&mut reader, file_len)?;
                        self.map.remove(&key);
                    }

                    CLEAR => self.map.clear(),

                    other => {
                        return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("unknown record tag {other}"),
                        ));
                    }
                }
                Ok(())
            })();

            match result {
                Ok(()) => {},
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    OpenOptions::new().write(true).open(&self.save_directory)?.set_len(record_start)?;
                    break;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }        

        Ok(())
    }

    //read only
    //param format is (name: type, name: type)
    fn get(&self, key: &str) -> Option<&String> {
        self.map.get(key)
    }

    //enters a value into the db
    fn put(&mut self, key: String, value: String) -> io::Result<()> {
        match self.log.write_all(&[PUT]) {
            Ok(_) => {},
            Err(_) => {
                println!("Failed to write to log within put cmd");
            }
        }

        match write_chunk(&mut self.log, &key) {
            Ok(_) => {},
            Err(_) => {
                println!("Failed to write to log within put cmd");
            }
        }

        match write_chunk(&mut self.log, &value) {
            Ok(_) => {},
            Err(_) => {
                println!("Failed to write to log within put cmd");
            }
        }

        match self.log.flush() {
            Ok(_) => {},
            Err(_) => {
                println!("Failed to flush write log");
            }
        }

        self.map.insert(key, value);
        Ok(())
    }

    //returns removed value
    fn del(&mut self, key: &str) -> io::Result<Option<String>> {
        if !self.map.contains_key(key) {
            return Ok(None);
        }

        self.log.write_all(&[DEL])?;

        match write_chunk(&mut self.log, key) {
            Ok(_) => {},
            Err(_) => {
                println!("Failed to write deletion");
            },
        }

        match self.log.flush() {
            Ok(_) => {},
            Err(_) => {
                println!("Failed to flush deletion log");
            },
        }

        Ok(self.map.remove(key))
    }

    //clears the map
    fn clear(&mut self) -> io::Result<()> {
        self.log.write_all(&[CLEAR])?;
        self.log.flush()?;

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
