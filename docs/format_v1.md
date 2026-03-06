# Ember File Format Specification (v1)

This document defines the on-disk format for Ember table files (version 1).

Each table in Ember is stored as a single file with the extension `.eb`.

Example:

    ./data/users.eb

This specification applies to version 1 of the format only.


---

## 1. File Identity

- File extension: `.eb`
- Magic bytes: ASCII `EMBR`
- Version: `u16`
- Endianness: All multi-byte integers are encoded **little-endian**

All Ember table files MUST begin with the magic bytes `EMBR`.

If the magic bytes do not match, the file MUST be rejected as:
    "Not an Ember table file"


---

## 2. File Layout Overview

An Ember table file consists of:

    +------------------+
    | Header           |
    +------------------+
    | Record 1         |
    +------------------+
    | Record 2         |
    +------------------+
    | ...              |
    +------------------+

The file is append-only after header creation.


---

## 3. Header Layout

The header is written exactly once when the table is created.

Header fields (in order):

1. Magic (4 bytes)
   - ASCII: `EMBR`

2. Version (u16)
   - Format version number
   - For v1: value MUST be `1`

3. Schema Length (u32)
   - Length in bytes of the schema JSON

4. Schema Bytes (variable)
   - UTF-8 encoded JSON schema
   - Length defined by Schema Length

5. Header Checksum (u32)
   - CRC32 checksum
   - Covers all header bytes from:
       Magic through Schema Bytes
   - Does NOT include the checksum field itself


### Header Validation Rules

- Magic MUST equal `EMBR`
- Version MUST equal `1`
- CRC32 MUST match
- If validation fails → table open MUST fail


---

## 4. Schema Encoding (v1)

Schema is encoded as UTF-8 JSON.

Example schema JSON:

```json
{
  "columns": [
    { "name": "id", "type": "INT" },
    { "name": "name", "type": "TEXT" }
  ]
}

---

5. Record Layout (Append-Only)

Each appended record has the following layout:

1. Record Length (u32)
   - Length in bytes of record_bytes

2. Record Bytes (variable)
   - Encoded row payload

3. Record Checksum (u32)
   - CRC32 checksum of:
      - record_length encoded as u32 little-endian
      - followed by record_bytes
   - The checksum does not include the checksum field itself

----
6. Row Encoding (v1)

Row values are encoded in schema column order.

For each column:

**INT**
- encoded as i64
- 8 bytes
- little-endian

**TEXT**

- encoded as: text length (u32, little-endian)
- UTF-8 text bytes