use std::collections::HashSet;
use std::fs::{self, create_dir_all};
use std::path::PathBuf;
use std::io::Write;
use crc32fast::Hasher;
use serde::Serialize;

use crate::error::{EmberError, EMPTY_NAME, INVALID_CHARACTERS, INVALID_START_CHARACTER, Kind};

const VERSION: u16 = 1;
const MAGIC: &str = "EMBR";

pub mod error;

pub type EmberResult<T> = std::result::Result<T, EmberError>;

pub struct Ember {
    data_directory: PathBuf,
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub enum ColumnType {
    INT,
    TEXT
}

#[derive(Serialize, PartialEq, Debug)]
struct Column {
    #[serde(rename = "name")]
    col_name: String,
    #[serde(rename = "type")]
    col_type: ColumnType
}

#[derive(Serialize)]
struct Schema {
    columns: Vec<Column>
}

impl Ember {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            data_directory: base_path.join("data")
        }
    }

    pub fn init(&self) -> EmberResult<()> {
        create_dir_all(&self.data_directory)
            .map_err(|e| EmberError::io(e, format!("creating data directory")))?;
        Ok(())
    }

    fn validate_name(name: &str, kind: Kind) -> EmberResult<()> {
        if name.is_empty() {
            return Err(EmberError::InvalidName{name: name.to_string(), kind: kind, reason: EMPTY_NAME.to_string()});
        }
        let first_char = name.chars().next().unwrap();
        if !first_char.is_alphabetic() && first_char != '_' {
            return Err(EmberError::InvalidName{name: name.to_string(), kind: kind, reason: INVALID_START_CHARACTER.to_string()});
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(EmberError::InvalidName{name: name.to_string(), kind: kind, reason: INVALID_CHARACTERS.to_string()});
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
                    _ => Err(EmberError::UnknownColumnType{col_type: column_type.to_string() }),
                }?;

                if schema_set.contains(column_name) {
                    return Err(EmberError::ColumnAlreadyExists { name: column_name.to_string() });
                }

                schema_set.insert(column_name.to_string());
                schema_list.push(Column{col_name: column_name.to_string(),col_type: valid_col_type});

            } else {
                return Err(EmberError::InvalidSchemaToken { token: val});
            }
        }

        Ok(schema_list)
    }

    fn calculate_crc32(data:&[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(data);
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
            return Err(EmberError::TableAlreadyExists { table: table_name.to_string() });
        }

        let schema_list = Self::validate_and_extract_schema(schema)?;
        let mut table = fs::File::create_new(table_path)
            .map_err(|e| EmberError::io(e, format!("creating table file '{}'", table_name)))?;
        let serialized_schema = serde_json::to_string(&Schema{columns: schema_list})
            .map_err(|e| EmberError::json(e, format!("serializing schema for table '{}'", table_name)))?;
        let schema_len: u32 = serialized_schema.len() as u32;
        
        let header_bytes = [
            MAGIC.as_bytes(),
            &VERSION.to_le_bytes(),
            &schema_len.to_le_bytes(),
            serialized_schema.as_bytes()
        ].concat();
        let checksum = Self::calculate_crc32(&header_bytes);

        table.write_all(&header_bytes)
            .map_err(|e| EmberError::io(e, format!("creating table file '{}'", table_name)))?;
        table.write_all(&checksum.to_le_bytes())
            .map_err(|e| EmberError::io(e, format!("creating table file '{}'", table_name)))?;

        Ok(())
    }
}


#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn magic_string_length() {
        assert_eq!(MAGIC.as_bytes().len(),4);
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
            Ember::validate_and_extract_schema(vec!["user:text".to_string(),"age:int".to_string(),"age:INT".to_string()]),
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
                Err(EmberError::InvalidName { name: n, kind: k, reason: r }) => {
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