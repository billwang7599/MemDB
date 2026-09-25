use clap::{Parser, Subcommand};
use memdb::document::{Document, DocumentId};
use memdb::storage::{Log, Operation};
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "memdb", about = "A small append-only document store")]
struct Cli {
    log_path: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    New, // new log
    // document operations
    Put { id: u64, content: Vec<String> },
    Get { id: u64 },
    Delete { id: u64 },
}

// one line typed into the shell, parsed the same way as the command line
#[derive(Parser)]
#[command(no_binary_name = true)]
struct ShellLine {
    #[command(subcommand)]
    command: Command,
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
        Some(Command::New) => {
            Log::create(&cli.log_path)?;
            println!("created {}", cli.log_path);
        }
        Some(Command::Put { id, content }) => {
            let mut log: Log = open_existing(&cli.log_path)?;
            let content: String = if content.is_empty() {
                let mut buf: String = String::new();
                io::stdin().read_to_string(&mut buf)?;
                buf
            } else {
                content.join(" ")
            };
            put(&mut log, id, content)?;
        }
        Some(Command::Get { id }) => {
            let mut log: Log = open_existing(&cli.log_path)?;
            println!("{}", get(&mut log, id)?);
        }
        Some(Command::Delete { id }) => {
            let mut log: Log = open_existing(&cli.log_path)?;
            delete(&mut log, id)?;
        }
        None => {
            let mut log: Log = open_existing(&cli.log_path)?;
            shell(&mut log)?;
        }
    }
    Ok(())
}

fn put(log: &mut Log, id: u64, content: String) -> io::Result<()> {
    let doc: Document = Document::new(DocumentId(id), content);
    log.append(&Operation::Insert(doc))?;
    Ok(())
}

fn get(log: &mut Log, id: u64) -> io::Result<String> {
    match log.get(DocumentId(id))? {
        Some(doc) => Ok(doc.content),
        None => Err(not_found(id)),
    }
}

fn delete(log: &mut Log, id: u64) -> io::Result<()> {
    log.append(&Operation::Delete(DocumentId(id)))?;
    Ok(())
}

// reads commands from stdin until `exit` or end of input; the log stays open throughout
fn shell(log: &mut Log) -> io::Result<()> {
    // only show a prompt when a person is typing, so piped input gives clean output
    let interactive: bool = io::stdin().is_terminal();
    if interactive {
        println!("memdb shell: `help` for commands, `exit` or Ctrl-D to quit");
    }
    let mut line: String = String::new();

    loop {
        if interactive {
            print!("memdb> ");
            io::stdout().flush()?;
        }

        line.clear();
        if io::stdin().read_line(&mut line)? == 0 {
            if interactive {
                println!();
            }
            return Ok(());
        }

        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(&first) = words.first() else {
            continue;
        };
        if first == "exit" || first == "quit" {
            return Ok(());
        }

        match ShellLine::try_parse_from(words) {
            Ok(ShellLine { command }) => {
                if let Err(e) = run_shell_command(log, command) {
                    eprintln!("error: {e}");
                }
            }
            // clap prints help to stdout and parse errors to stderr
            Err(e) => {
                let _ = e.print();
            }
        }
    }
}

fn run_shell_command(log: &mut Log, command: Command) -> io::Result<()> {
    match command {
        Command::New => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a log is already open",
        )),
        Command::Put { id, content } => {
            // stdin carries the commands here, so content must be on the line
            if content.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: put <id> <content>",
                ));
            }
            put(log, id, content.join(" "))
        }
        Command::Get { id } => {
            println!("{}", get(log, id)?);
            Ok(())
        }
        Command::Delete { id } => delete(log, id),
    }
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
