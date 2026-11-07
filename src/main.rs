use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, ErrorKind, Read, Write};

fn read_chunk<R: Read>(reader: &mut R) -> io::Result<Option<String>> {
    let mut len_buf = [0u8; 4];

    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let len = u32::from_le_bytes(len_buf) as usize;

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf);

    String::from_utf8(buf).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

//store struct
struct Store {
    //the hashmap storage itself
    map: HashMap<String, String>,

    //file location for saving and loading from
    save_directory: String,
}

impl Store {
    //constructor
    //called using Store::new(...)
    fn new() -> Self {
        Store {
            map: HashMap::new(),
            save_directory: "save.bin".to_string(),
        }
    }

    fn save(&self) -> io::Result<()>  {
        //write to binary file
        let file = File::create(&self.save_directory)?;
        let mut writer = BufWriter::new(file);

        for (key, value) in &self.map {
            writer.write_all(&(key.len() as u32).to_le_bytes())?;
            writer.write_all(&key.as_bytes())?;
            writer.write_all(&(value.len() as u32).to_le_bytes())?;
            writer.write_all(&value.as_bytes())?;
        }

        writer.flush()?;

        Ok(())
    }

    fn load(&mut self) -> io::Result<()> {
        let file = match File::open(&self.save_directory) {
            Ok(f) => f,
            
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };

        let mut reader = BufReader::new(file);

        while let Some(key) = read_chunk(&mut reader)? {
            let val = read_chunk(&mut reader)?.ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "Key without a value")
            })?;

            self.map.insert(key, val);
        }

        Ok(())
    }

    //read only
    //param format is (name: type, name: type)
    fn get(&self, key: &str) -> Option<&String> {
        self.map.get(key)
    }

    //enters a value into the db
    fn put(&mut self, key: String, value: String) {
        self.map.insert(key, value);
    }

    //returns removed value
    fn del(&mut self, key: &str) -> Option<String> {
        self.map.remove(key)
    }

    //clears the map
    fn clear(&mut self) {
        self.map.clear();
    }
}

fn main() {
    let mut store = Store::new();

    io::stdout().flush().expect("Failed to flush stdout");

    store.load().expect("Failed to load save file.");

    let mut conf_clear = false;

    loop {
        print!("> ");
        io::stdout().flush().expect("Failed to flush stdout");

        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input).expect("Failed to read line");

        if(bytes==0){
            break;
        }

        let args: Vec<&str> = input.split_whitespace().collect();

        //replacement for if/else chains
        match args.as_slice() {
            ["put", key, value] => {
                store.put(key.to_string(), value.to_string());
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

            ["del", key] => match store.del(key) {
                Some(v) => {
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
                if(!conf_clear){
                    println!("Are you sure you want to clear all keys? Retype 'clear' to confirm");
                    conf_clear=true;
                } else {
                    store.clear();
                    conf_clear=false;
                    println!("Cleared all keys in the store successfully");
                }
            }

            [] => {}//empty line
            _ => println!("Unknown commmand")
        }
    }

    store.save().expect("Failed to save to file");

}
