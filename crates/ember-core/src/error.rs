use std::{fmt,io};

pub const INVALID_START_CHARACTER: &str = "Must start with a letter or underscore";
pub const INVALID_CHARACTERS: &str = "Must contain only alphanumeric characters or underscores.";
pub const EMPTY_NAME:  &str = "Name must not be empty";

#[derive(Debug)]
pub enum Kind {
    Column,
    Table
}

#[derive(Debug)]
pub enum EmberError {
    InvalidName {
        name: String,
        kind: Kind,
        reason: String
    },
    EmptySchema,
    InvalidSchemaToken {
        token: String,
    },
    UnknownColumnType {
        col_type: String,
    },
    ColumnAlreadyExists {
        name: String,
    },
    TableAlreadyExists {
        table: String,
    },
    NotInitialized,
    Io {
        err: io::Error,
        context: String,
    },
    Json {
        err: serde_json::Error,
        context: String,
    },
}

impl fmt::Display for EmberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmberError::InvalidName { name,kind , reason} => {
                write!(f, "Invalid {} name '{}': {}", kind, name, reason)
            }
            EmberError::EmptySchema => {
                write!(f,"Schema cannot be empty!")
            }
            EmberError::InvalidSchemaToken { token } => {
                write!(f, "Invalid column definition: {}", token)
            }
            EmberError::UnknownColumnType { col_type } => {
                write!(f, "Unknown column type: {}", col_type)
            }
            EmberError::ColumnAlreadyExists { name } => {
                write!(f, "Duplicate column: {}", name)
            }
            EmberError::TableAlreadyExists { table } => {
                write!(f, "Table '{}' already exists", table)
            }
            EmberError::NotInitialized => {
                write!(f, "Ember project is not initialized")
            }
            EmberError::Io { err, context } => {
                write!(f, "IO error during {}: {}", context, err)
            }
            EmberError::Json { err, context } => {
                write!(f, "JSON error during {}: {}", context, err)
            }
        }
    }
}

impl EmberError {
    pub fn io<E: Into<io::Error>, S: Into<String>>(err: E, context: S) -> Self {
        EmberError::Io { err: err.into(), context: context.into() }
    }

    pub fn json<E: Into<serde_json::Error>, S: Into<String>>(err: E, context: S) -> Self {
        EmberError::Json { err: err.into(), context: context.into() }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            EmberError::InvalidName { .. } => 2,
            EmberError::EmptySchema => 2,
            EmberError::InvalidSchemaToken { .. } => 2,
            EmberError::UnknownColumnType { .. } => 2,
            EmberError::ColumnAlreadyExists { .. } => 2,
            EmberError::TableAlreadyExists { .. } => 2,
            EmberError::NotInitialized => 2,
            EmberError::Io { .. } => 1,
            EmberError::Json { .. } => 1,
        }
    }
}

impl std::error::Error for EmberError {}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Column => write!(f,"column"),
            Kind::Table => write!(f,"table"),
        }
    }
}
