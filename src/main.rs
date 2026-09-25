use clap::{Parser, Subcommand};
use memdb::document::{Document, DocumentId};
use memdb::storage::{Log, Operation};
use std::io::{self, Read};
use std::path::Path;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "memdb", about = "A small append-only document store")]
struct Cli {
    log_path: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    New, // new log
    // document operations
    Put { id: u64, content: Option<String> },
    Get { id: u64 },
    Delete { id: u64 },
}

fn main() -> ExitCode {
    let cli: Cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> io::Result<()> {
    match cli.command {
        Command::New => {
            Log::create(&cli.log_path)?;
            println!("created {}", cli.log_path);
        }
        Command::Put { id, content } => {
            let mut log: Log = open_existing(&cli.log_path)?;
            let content: String = match content {
                Some(content) => content,
                None => {
                    let mut buf: String = String::new();
                    io::stdin().read_to_string(&mut buf)?;
                    buf
                }
            };
            let doc: Document = Document::new(DocumentId(id), content);
            log.append(&Operation::Insert(doc))?;
        }
        Command::Get { id } => {
            let mut log: Log = open_existing(&cli.log_path)?;
            match log.get(DocumentId(id))? {
                Some(doc) => println!("{}", doc.content),
                None => return Err(not_found(id)),
            }
        }
        Command::Delete { id } => {
            let mut log: Log = open_existing(&cli.log_path)?;
            log.append(&Operation::Delete(DocumentId(id)))?;
        }
    }
    Ok(())
}

fn open_existing(path: &str) -> io::Result<Log> {
    if !Path::new(path).exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no log file at {path} (create one with `new`)"),
        ));
    }
    Log::open(path)
}

fn not_found(id: u64) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("no document with id {id}"))
}
