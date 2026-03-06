use clap::{Parser, Subcommand};
use ember_core::{Ember, EmberResult, Value, error::EmberError};
use std::env;

#[derive(Parser)]
#[command(name = "ember")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    CreateTable {
        table_name: String,
        schema: Vec<String>,
    },
    Insert {
        table_name: String,
        record: Vec<String>,
    },
    Scan {
        table_name: String,
    },
}

fn run() -> EmberResult<()> {
    let path = env::current_dir().map_err(|e| EmberError::io(e, "getting current directory"))?;
    let ember = Ember::new(path);

    let cli = Cli::parse();
    match cli.command {
        Commands::Init => ember.init()?,
        Commands::CreateTable { table_name, schema } => ember.create_table(&table_name, schema)?,
        Commands::Insert { table_name, record } => ember.insert(&table_name, record)?,
        Commands::Scan { table_name } => {
            let (schema, rows) = ember.scan(&table_name)?;
            let columns: Vec<&str> = schema.iter().map(|c| c.col_name.as_str()).collect();
            println!("{}", columns.join("\t"));

            for row in rows {
                let row_strs: Vec<String> = row
                    .iter()
                    .map(|val| match val {
                        Value::Int(n) => n.to_string(),
                        Value::Text(s) => s.clone(),
                    })
                    .collect();
                println!("{}", row_strs.join("\t"));
            }
        }
    };

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(e.exit_code());
    }
}
