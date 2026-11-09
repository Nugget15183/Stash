use std::{io::{self, Write},};
use crate::{Store};

pub fn terminal() {
    let repair = std::env::args().any(|a| a == "--repair");

    let mut store = if repair {
        let (store, _) = match Store::open_repair("save.bin") {
            Ok(s) => s,

            Err(e) => {
                eprintln!("Failed to open store: {e}");
                std::process::exit(1);
            }
        };

        store
    } else {
        let store = match Store::new("save.bin") {
            Ok(s) => s,

            Err(e) => {
                eprintln!("Failed to open store: {e}");
                std::process::exit(1);
            }
        };

        store
    };

    io::stdout().flush().expect("Failed to flush stdout");

    let mut conf_clear = false;

    loop {
        print!("> ");
        io::stdout().flush().expect("Failed to flush stdout");

        let mut input = String::new();
        let bytes = io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        if bytes == 0 {
            break;
        }

        let args: Vec<&str> = input.split_whitespace().collect();

        //replacement for if/else chains
        match args.as_slice() {
            ["put", key, value] => {
                store
                    .put(key.to_string(), value.to_string())
                    .expect("Failed to write key to store");
                println!("Set key {key} to value {value} successfully");
            }

            ["get", key] => match store.get(&key.to_string()) {
                Some(v) => {
                    println!("Value at key {key}: {v}")
                }
                None => {
                    println!("Key {key} not found.")
                }
            },

            ["del", key] => match store.del(key).expect("Failed to write deletion of key") {
                Some(_) => {
                    println!("Key {key} deleted successfully")
                }

                None => {
                    println!("Key {key} not found")
                }
            },

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
                    conf_clear = true;
                } else {
                    match store.clear() {
                        Ok(_) => {
                            println!("Cleared all keys in the store successfully");
                        }
                        Err(_) => {
                            println!("Failed to clear store.");
                        }
                    }
                    conf_clear = false;
                }
            }

            ["compact"] => match store.compact() {
                Ok(()) => println!("Compacted log successfully"),
                Err(e) => println!("Compation failed: {e}"),
            }

            [] => {} //empty line
            _ => println!("Unknown commmand"),
        }
    }
}
