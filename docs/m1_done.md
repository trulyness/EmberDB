# Milestone 1 — Definition of Done

Milestone 1 focuses on:

- Table storage
- Append-only records
- Persistence
- File format correctness
- CLI integration

This milestone is complete ONLY when all items below are satisfied.


---

## Storage & Persistence

- [ ] `ember init` creates a `./data` directory
- [ ] `ember create-table users id:int name:text` creates:
      ./data/users.eb
- [ ] Header is written exactly once
- [ ] Schema is stored as JSON
- [ ] Header checksum is validated on open
- [ ] Records are append-only


---

## Insert & Scan

- [ ] `ember insert users 1 "Adam"` appends a record
- [ ] Multiple inserts append sequentially
- [ ] `ember scan users` prints all rows
- [ ] Restarting the process preserves data
- [ ] Scan stops safely if encountering partial record


---

## Validation & Safety

- [ ] Opening a file with incorrect magic fails
- [ ] Opening a file with wrong version fails
- [ ] Corrupted header checksum fails
- [ ] Corrupted record checksum fails gracefully
- [ ] Incorrect column count is rejected
- [ ] Incorrect type is rejected


---

## Engineering Quality

- [ ] `cargo test` passes
- [ ] At least 5 meaningful tests exist:
      - schema roundtrip
      - row roundtrip
      - header checksum validation
      - record checksum validation
      - torn write handling
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo fmt` clean
- [ ] README contains:
      - File layout diagram
      - Demo commands
      - Design explanation


---

## Demo Script (Manual Validation)

The following must work end-to-end:

    ember init
    ember create-table users id:int name:text
    ember insert users 1 "Adam"
    ember insert users 2 "Coco"
    ember scan users

Then restart program:

    ember scan users

Output must still show both rows.


---

## Milestone 1 Success Criteria

Milestone 1 is complete when:

- The format is stable
- Data persists across restarts
- Corruption is handled safely
- The system feels production-conscious
- You are confident in the file format

No feature creep beyond this scope.