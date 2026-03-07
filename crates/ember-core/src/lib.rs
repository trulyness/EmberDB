use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions, create_dir_all};
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::{EMPTY_NAME, EmberError, INVALID_CHARACTERS, INVALID_START_CHARACTER, Kind};

const VERSION: u16 = 1;
const MAGIC: &str = "EMBR";
const MAX_SCHEMA_SIZE: usize = 64 * 1024;

pub mod error;

pub type EmberResult<T> = std::result::Result<T, EmberError>;

pub type Row = Vec<Value>;

pub struct Ember {
    data_directory: PathBuf,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub enum ColumnType {
    INT,
    TEXT,
}

pub enum Value {
    Int(i64),
    Text(String),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Column {
    #[serde(rename = "name")]
    pub col_name: String,
    #[serde(rename = "type")]
    col_type: ColumnType,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Schema {
    columns: Vec<Column>,
}

impl Ember {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            data_directory: base_path.join("data"),
        }
    }

    pub fn init(&self) -> EmberResult<()> {
        create_dir_all(&self.data_directory)
            .map_err(|e| EmberError::io(e, format!("creating data directory")))?;
        Ok(())
    }

    fn validate_name(name: &str, kind: Kind) -> EmberResult<()> {
        if name.is_empty() {
            return Err(EmberError::InvalidName {
                name: name.to_string(),
                kind: kind,
                reason: EMPTY_NAME.to_string(),
            });
        }
        let first_char = name.chars().next().unwrap();
        if !first_char.is_alphabetic() && first_char != '_' {
            return Err(EmberError::InvalidName {
                name: name.to_string(),
                kind: kind,
                reason: INVALID_START_CHARACTER.to_string(),
            });
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(EmberError::InvalidName {
                name: name.to_string(),
                kind: kind,
                reason: INVALID_CHARACTERS.to_string(),
            });
        }
        Ok(())
    }

    fn validate_and_extract_schema(schema: Vec<String>) -> EmberResult<Vec<Column>> {
        if schema.is_empty() {
            return Err(EmberError::EmptySchema);
        }

        let mut schema_set: HashSet<String> = HashSet::new();
        let mut schema_list: Vec<Column> = Vec::new();

        for val in schema {
            if let Some((column_name, column_type)) = val.split_once(':') {
                let column_name = column_name.trim();
                Self::validate_name(column_name, Kind::Column)?;
                let column_type = column_type.trim().to_ascii_uppercase();
                let valid_col_type = match column_type.as_str() {
                    "INT" => Ok(ColumnType::INT),
                    "TEXT" => Ok(ColumnType::TEXT),
                    _ => Err(EmberError::UnknownColumnType {
                        col_type: column_type.to_string(),
                    }),
                }?;

                if schema_set.contains(column_name) {
                    return Err(EmberError::ColumnAlreadyExists {
                        name: column_name.to_string(),
                    });
                }

                schema_set.insert(column_name.to_string());
                schema_list.push(Column {
                    col_name: column_name.to_string(),
                    col_type: valid_col_type,
                });
            } else {
                return Err(EmberError::InvalidSchemaToken { token: val });
            }
        }

        Ok(schema_list)
    }

    fn calculate_crc32(slices: &[&[u8]]) -> u32 {
        let mut hasher = Hasher::new();
        for slice in slices {
            hasher.update(slice);
        }
        hasher.finalize()
    }

    pub fn create_table(&self, table_name: &str, schema: Vec<String>) -> EmberResult<()> {
        if !self.data_directory.exists() {
            return Err(EmberError::NotInitialized);
        }
        let table_name = table_name.trim();
        Self::validate_name(table_name, Kind::Table)?;
        let table_path = self.data_directory.join(table_name).with_extension("eb");
        if table_path.exists() {
            return Err(EmberError::TableAlreadyExists {
                table: table_name.to_string(),
            });
        }

        let schema_list = Self::validate_and_extract_schema(schema)?;
        let mut table = fs::File::create_new(table_path)
            .map_err(|e| EmberError::io(e, format!("creating table file '{}'", table_name)))?;
        let serialized_schema = serde_json::to_string(&Schema {
            columns: schema_list,
        })
        .map_err(|e| {
            EmberError::json(e, format!("serializing schema for table '{}'", table_name))
        })?;
        let schema_len: u32 = serialized_schema.len() as u32;

        let header_bytes = [
            MAGIC.as_bytes(),
            &VERSION.to_le_bytes(),
            &schema_len.to_le_bytes(),
            serialized_schema.as_bytes(),
        ];

        let checksum = Self::calculate_crc32(&header_bytes);

        for byte in &header_bytes {
            table
                .write_all(byte)
                .map_err(|e| EmberError::io(e, format!("creating table file '{}'", table_name)))?;
        }

        table
            .write_all(&checksum.to_le_bytes())
            .map_err(|e| EmberError::io(e, format!("creating table file '{}'", table_name)))?;

        Ok(())
    }

    fn read_table_header(&self, file: &mut File, table: String) -> EmberResult<Vec<Column>> {
        // Read fixed header
        let mut fixed_header = [0u8; 10];
        file.read_exact(&mut fixed_header)
            .map_err(|e| EmberError::io(e, format!("reading header for '{}'", table)))?;

        let magic = &fixed_header[0..4];
        if magic != MAGIC.as_bytes() {
            return Err(EmberError::TableCorrupted { table });
        }

        let schema_len = u32::from_le_bytes(fixed_header[6..10].try_into().map_err(|_| {
            EmberError::TableCorrupted {
                table: table.clone(),
            }
        })?);

        if schema_len as usize > MAX_SCHEMA_SIZE {
            return Err(EmberError::TableCorrupted { table });
        }

        // Read schema bytes
        let mut schema_bytes = vec![0u8; schema_len as usize];
        file.read_exact(&mut schema_bytes)
            .map_err(|e| EmberError::io(e, format!("reading schema for '{}'", table)))?;

        // Read stored checksum
        let mut stored_checksum_bytes = [0u8; 4];
        file.read_exact(&mut stored_checksum_bytes)
            .map_err(|e| EmberError::io(e, format!("reading checksum for '{}'", table)))?;
        let stored_checksum = u32::from_le_bytes(stored_checksum_bytes);

        // Compute checksum
        let checksum = Self::calculate_crc32(&[&fixed_header, &schema_bytes]);

        if checksum != stored_checksum {
            return Err(EmberError::TableCorrupted { table });
        }

        let version = u16::from_le_bytes(fixed_header[4..6].try_into().unwrap());
        if version != VERSION {
            return Err(EmberError::TableCorrupted { table });
        }

        // Deserialize schema
        let schema: Schema = serde_json::from_slice(&schema_bytes)
            .map_err(|e| EmberError::json(e, format!("decoding schema for '{}'", table)))?;

        Ok(schema.columns)
    }

    pub fn insert(&self, table_name: &str, record: Vec<String>) -> EmberResult<()> {
        if !self.data_directory.exists() {
            return Err(EmberError::NotInitialized);
        }
        let table_name = table_name.trim();
        let table_path = self.data_directory.join(table_name).with_extension("eb");
        if !table_path.exists() {
            return Err(EmberError::TableDoesNotExist {
                table: table_name.to_string(),
            });
        }

        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(false)
            .open(&table_path)
            .map_err(|e| EmberError::io(e, format!("opening table '{}'", table_name)))?;

        let schema = self.read_table_header(&mut file, table_name.to_string())?;

        if record.len() != schema.len() {
            return Err(EmberError::ColumnCountMismatch {
                expected_count: schema.len(),
                provided_count: record.len(),
            });
        }

        let mut row_bytes: Vec<u8> = Vec::new();

        for (col, val) in schema.iter().zip(&record) {
            match col.col_type {
                ColumnType::INT => {
                    let sanitized_val =
                        val.parse::<i64>()
                            .map_err(|_| EmberError::IncompatibleDataTypes {
                                val: val.to_string(),
                                col_type: ColumnType::INT,
                            })?;

                    row_bytes.extend(sanitized_val.to_le_bytes());
                }
                ColumnType::TEXT => {
                    let bytes = val.as_bytes();
                    let len = bytes.len() as u32;

                    row_bytes.extend(&len.to_le_bytes());
                    row_bytes.extend(val.as_bytes());
                }
            }
        }

        let row_len = row_bytes.len() as u32;

        file.write_all(&row_len.to_le_bytes())
            .map_err(|e| EmberError::io(e, format!("inserting row for '{}'", table_name)))?;
        file.write_all(&row_bytes)
            .map_err(|e| EmberError::io(e, format!("inserting row for '{}'", table_name)))?;

        let checksum = Self::calculate_crc32(&[&row_len.to_le_bytes(), &row_bytes]);

        file.write_all(&checksum.to_le_bytes())
            .map_err(|e| EmberError::io(e, format!("inserting row for '{}'", table_name)))?;

        Ok(())
    }

    fn read_table_rows(
        &self,
        file: &mut File,
        table: String,
        schema: &Vec<Column>,
    ) -> EmberResult<Vec<Row>> {
        let mut rows = Vec::new();
        loop {
            let mut row_len_buf = [0u8; 4];
            match file.read_exact(&mut row_len_buf) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(EmberError::io(e, format!("reading rows for '{}'", table))),
            }

            let row_len = u32::from_le_bytes(row_len_buf) as usize;

            let mut row_bytes = vec![0u8; row_len];
            file.read_exact(&mut row_bytes)
                .map_err(|e| EmberError::io(e, format!("reading rows for '{}'", table)))?;

            // read stored checksum
            let mut stored_checksum_bytes = [0u8; 4];
            file.read_exact(&mut stored_checksum_bytes)
                .map_err(|e| EmberError::io(e, format!("reading row for '{}'", table)))?;
            let stored_checksum = u32::from_le_bytes(stored_checksum_bytes);

            // verify checksum
            let checksum = Self::calculate_crc32(&[&row_len_buf, &row_bytes]);
            if checksum != stored_checksum {
                return Err(EmberError::TableCorrupted {
                    table: table.clone(),
                });
            }

            let mut cursor = 0;
            let mut row: Row = Vec::new();
            for col in schema {
                match col.col_type {
                    ColumnType::INT => {
                        if cursor + 8 > row_bytes.len() {
                            return Err(EmberError::TableCorrupted {
                                table: table.clone(),
                            });
                        }

                        let num =
                            i64::from_le_bytes(row_bytes[cursor..cursor + 8].try_into().map_err(
                                |_| EmberError::TableCorrupted {
                                    table: table.clone(),
                                },
                            )?);
                        row.push(Value::Int(num));
                        cursor += 8;
                    }
                    ColumnType::TEXT => {
                        if cursor + 4 > row_bytes.len() {
                            return Err(EmberError::TableCorrupted {
                                table: table.clone(),
                            });
                        }
                        let len =
                            u32::from_le_bytes(row_bytes[cursor..cursor + 4].try_into().map_err(
                                |_| EmberError::TableCorrupted {
                                    table: table.clone(),
                                },
                            )?) as usize;
                        cursor += 4;

                        if cursor + len > row_bytes.len() {
                            return Err(EmberError::TableCorrupted {
                                table: table.clone(),
                            });
                        }

                        let text = String::from_utf8(row_bytes[cursor..cursor + len].to_vec())
                            .map_err(|_| EmberError::TableCorrupted {
                                table: table.clone(),
                            })?;
                        row.push(Value::Text(text));
                        cursor += len;
                    }
                }
            }

            if cursor != row_bytes.len() {
                return Err(EmberError::TableCorrupted {
                    table: table.clone(),
                });
            }

            rows.push(row);
        }

        return Ok(rows);
    }

    pub fn scan(&self, table_name: &str) -> EmberResult<(Vec<Column>, Vec<Row>)> {
        if !self.data_directory.exists() {
            return Err(EmberError::NotInitialized);
        }
        let table_name = table_name.trim();
        let table_path = self.data_directory.join(table_name).with_extension("eb");
        if !table_path.exists() {
            return Err(EmberError::TableDoesNotExist {
                table: table_name.to_string(),
            });
        }

        let mut file = OpenOptions::new()
            .read(true)
            .create(false)
            .open(&table_path)
            .map_err(|e| EmberError::io(e, format!("opening table '{}'", table_name)))?;

        let schema = self.read_table_header(&mut file, table_name.to_string())?;

        let rows = self.read_table_rows(&mut file, table_name.to_string(), &schema)?;

        Ok((schema, rows))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn magic_string_length() {
        assert_eq!(MAGIC.as_bytes().len(), 4);
    }

    #[test]
    fn test_validate_and_extract_schema_error_cases() {
        assert!(matches!(
            Ember::validate_and_extract_schema(vec![]),
            Err(EmberError::EmptySchema)
        ));

        assert!(matches!(
            Ember::validate_and_extract_schema(vec!["token".to_string()]),
            Err(EmberError::InvalidSchemaToken { .. })
        ));

        assert!(matches!(
            Ember::validate_and_extract_schema(vec!["user:jpg".to_string()]),
            Err(EmberError::UnknownColumnType { .. })
        ));

        assert!(matches!(
            Ember::validate_and_extract_schema(vec![
                "user:text".to_string(),
                "age:int".to_string(),
                "age:INT".to_string()
            ]),
            Err(EmberError::ColumnAlreadyExists { .. })
        ));
    }

    #[test]
    fn test_validate_and_extract_schema_success() {
        let result = Ember::validate_and_extract_schema(vec![
            "user:Text".into(),
            "age:iNt".into(),
            "Age:INt".into(),
            "AGE:int".into(),
        ])
        .expect("schema should be valid");

        assert_eq!(
            result,
            vec![
                Column {
                    col_name: "user".into(),
                    col_type: ColumnType::TEXT
                },
                Column {
                    col_name: "age".into(),
                    col_type: ColumnType::INT
                },
                Column {
                    col_name: "Age".into(),
                    col_type: ColumnType::INT
                },
                Column {
                    col_name: "AGE".into(),
                    col_type: ColumnType::INT
                }
            ]
        );
    }

    #[test]
    fn test_validate_name_error_cases() {
        let cases = vec![
            ("", Kind::Column, EMPTY_NAME),
            ("", Kind::Table, EMPTY_NAME),
            ("1ABC", Kind::Table, INVALID_START_CHARACTER),
            ("ABC@", Kind::Table, INVALID_CHARACTERS),
        ];

        for (name, kind, reason) in cases {
            let result = Ember::validate_name(name, kind);

            match result {
                Err(EmberError::InvalidName {
                    name: n,
                    kind: k,
                    reason: r,
                }) => {
                    assert_eq!(n, name);
                    assert_eq!(k, kind);
                    assert_eq!(r, reason);
                }
                _ => panic!("expected InvalidName error for input '{}'", name),
            }
        }
    }

    #[test]
    fn test_validate_name_success_cases() {
        let cases = vec![
            ("users", Kind::Table),
            ("_internal", Kind::Table),
            ("column1", Kind::Column),
            ("user_name", Kind::Column),
        ];

        for (name, kind) in cases {
            assert!(Ember::validate_name(name, kind).is_ok());
        }
    }
}
