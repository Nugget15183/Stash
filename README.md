# Stash
A small, persistent key-value storage system written purely in Rust. Stash keeps everything in memory with a `HashMap` and writes every modification to a to an append only, checksummed log. That way, your data can survive restarts, crashes, file corruption and truncation.

## Why did I build this?
I build it as a way to learn rust and also to understand how log structured storage engines work. Framing, Checksums, crash recovery, and repair.

## Features
- **Persistence**: every `put`, `del`, and `clear` is appended to `save.bin` before any in memory map changes.
- **Append only log**: state is rebuilt each run on startup by replaying the log.
- **CRC32 Checksums**: every record is checksummed, so corruption is detected and discarded automatically on startup *if* you are running in repair mode.
- **Repair vs Normal run modes**: By default the program is run in normal mode, when it detects corruption or a tear, it stops, preventing any unwanted discardation of the rest of the file. Vice versa, Repair mode automatically cleans up the file, but its not always recommended to run it in repair mode because you may end up discarding important data you might want.
- **Examples**: an interactive command line example is already setup to allow users to play around with the tool.
- **Few dependencies**: only [`crc32fast`](https://crates.io/crates/crc32fast) and [`chrono`](https://crates.io/crates/chrono) (for backup timestamps). 

## Getting Started
You need a rust toolchain ([rustup.rs]("https://rustup.rs")).

```sh
git clone https://github.com/Nugget15183/Stash
cd Stash
cargo run
```

Stash reads and writes `save.bin` in the directory you run it from. Run it from the project root so you always use the same file, and note that a backup is always created on run before you can modify any values.

To start in the aforementioned **Repair mode**:
```sh
cargo run -- --repair
```
> Note: The bare -- tells cargo to pass all the arguments into Stash instead of interpreting them itself

## Commands
`put <key> <value>` | Store a value under a key .
> Note: Using an existing key overwrites the value in the current key with the new value

`get <key>`| Returns the value at the key.

`del <key>`| Deletes the key.

`list`| Lists all keys with values.

`clear`| Clears all keys within storage.

`exit`| Stops the program

---
> Keys and Values both should be single words, no spaces inbetween or special characters.


## Example sessions
### Basic usage

```text
> put Username Guest
Set key Username to Guest successfully
> put Language Rust
Set key Language to Rust successfully
> get Username
Value at key Username: Guest
> get Color
Key color not found
> list
Username
Language
> del Language
Key Language deleted successfully
> del Language
Key Language not found
> exit
```
> list prints keys in arbitrary order, because they're stored in a `HashMap`

### Data surviving restarts
```text
> put counter 1
Set key counter to 1 successfully
> exit
```

Run `cargo run` again

```text
> get counter
Value at key counter: 1
```

### Clearing the store
```text
> put age 20
Value at key age: 20
> put name tom
Value at key name: tom
> put favorite_color blue
Value at key favorite_color: blue
> list
age
name
favorite_color
> clear
Are you sure you want to clear all keys? Retype 'clear' to confirm
> clear
Cleared all keys in the store successfully
>list
```
> No line after the `list` command because there is nothing to list

## Crash safety and corruption handling

Each record in `save.bin` has this layout
```text
[ payload_len: u32 ][ crc32: u32 ][ payload ]

payload = [ tag: u8 ][ chunk ][ chunk ]...
chunk = [ len: u32 ][ UTF-8 bytes ]

tag: 0 = PUT (key chunk, value chunk)
     1 = DEL (key chunk)
     2 = CLEAR (no chunks)
```

Each record is built in memory and written with a single `write_all` then flushed. On startup, Stash replays the log record by record and verifies each checksum before applying it. What happens depends on what it finds:

| Situation | Meaning | What Stash does |
| --- | --- | --- |
| Record is cut off | Torn Write, likely caused by a crash mid append | Discards it, gives a warning, and truncates the file
| Checksum fails on the **last** record | Probbably a torn write | Discards it, truncates file, gives warning
| Checksum fails on a record **in the middle** of the file | Real corruption | Refuses to open and reports byte offset
| Checksum passes but the payload doesnt parse | Bug or format mismatch | Refuses to open

Refusing to open is intentional. Since silently dropping data in the middle of a log can change everything after it, it needs a descision from **you**.

### Repair mode

```sh
cargo run -- --repair
```

Repair mode:
1. Copies save.bin to a timestampped backup next to it
2. Skips damaged record in the middle of the file
3. If two damaged records appear back to back then it discards everything from then on, since it cannot trust either's length
4. Rewrites the file through a temporary file and an atomic rename, so a crash cannot destroy the log

**Skipped records can change your data.** the log is a history, so a skipped `put` can bring back an older value for that key, a skipped `del` can bring back a deleted key, and a skipped `clear` is undone. Repair gives you the most consistent state that can be rebuilt from the surviving records, not always the state you had. **Keep the backups until you know for a fact you do not need them anymore.**

### Try it yourself
These PowerShell commands damage `save.bin` intentionally. **Copy `save.bin` beforehand**

---

Put atleast 3 single character keys and values (`put a 1`, `put b 2`, `put c 3`), exit, then pick a test.

Simulate a torn write by chopping off the last 3 bytes of the file:
```powershell
$b = [System.IO.File]::ReadAllBytes("save.bin")
$b[13] = $b[13] -bxor 0xFF
[System.IO.File]::WriteAllBytes("save.bin", $b)
```

A normal run should now refuse to open with `checksum mismatched in record at byte 0`. Running with `--repair` should create a backup, skip the damaged record, and open. Run once more normally to confirm the save was truly fixed.

## Using the store from code
```rust
let mut store = Store::new("data.bin")?;

store.put("name".to_string(), "tom".to_string())?;

if let Some(value) = store.get("name") {
    println!("name = {value}");
}

let removed = store.del("name")?; //returns old value, or None
store.clear()?;

//Open in repair mode and view what happened
let (store, report) = Store::open_repair("data.bin")?; 

println!("skipped {} damaged record(s)", report.bad_ranges.len());

if let Some(offset) = report.truncate_at {
    println!("Discarde everything from byte {offset}");
}
```

## Project Layout
```text
src/
    main.rs     Store itself, encoding, loading, put/get/del/ect
    load.rs     Record reader and LoadReport
    consts.rs   Record tags and size limits
    examples.rs The interactive terminal
```

## Limitations
- **The log never shrinks.** Overwritten and deleted entries stay in the file until compaction is implemented
- **Everything lives in memory.** The whole key set and all values are loaded to a `HashMap` at startup
- **Single process only.** Only ever access the log from one process at a time
- **Flush not fsync.** Writes are flushed to the OS after each record but not forced to disk, a power cut can lose a lot of records made from the last open to the current one.

## Ideas for future work
- Compaction (rewrite the log for live entries only)
- Magic number and version byte in each save
- TCP Server mode