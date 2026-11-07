use std::collections::{HashMap, vec_deque};
use std::collections::btree_map::Values;
use std::fs::File;
use std::fs::OpenOptions;
use std::{fs, vec};
use std::io::{self, BufRead, BufReader, BufWriter, Lines, Write};

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
            writer.write_all(&keybytes)?;
            writer.write_all(&kvsep)?;
            writer.write_all(&valbytes)?;
            writer.write_all(&linesep)?;
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

        for line in BufReader::new(file).lines() {
            let line = line?;

            if let Some((key, value)) = line.split_once(' ') {
                self.map.insert(key.to_string(), value.to_string());
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
