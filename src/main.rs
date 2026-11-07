use std::collections::HashMap;
use std::collections::btree_map::Values;
use std::io::{self, Write};

//store struct
struct Store {
    //the hashmap storage itself
    map: HashMap<String, String>,
}

impl Store {
    //constructor
    //called using Store::new(...)
    fn new() -> Self {
        Store {
            map: HashMap::new(),
        }
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
}

fn main() {
    let mut store = Store::new();

    let mut running = true;

    io::stdout().flush().expect("Failed to flush stdout");

    let mut input = String::new();

    loop {
        print!("> ");
        io::stdout().flush().expect("Failed to flush stdout.");

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
                println!("Set key {key} to value {value} successfully.");
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
                    println!("Key {key} deleted successfully.")
                }

                None => {
                    println!("Key {key} not found.")
                }
            } 

            ["exit"] => {
                break;
            }

            [] => {}//empty line
            _ => println!("Unknown commmand")
        }
    }

    /*
    store.put("Does this work".to_string(), "True".to_string());

    let result = store.get(&"Does this work".to_string());

    match store.get("Does this work") {
        Some(v) => println!("Working = {v}"),
        None => println!("Not found")
    }

    println!("{:?}", result)
    */
}
