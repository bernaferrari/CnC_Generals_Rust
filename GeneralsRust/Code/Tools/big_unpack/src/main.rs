use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "BIG archive utility (placeholder)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// List entries in a BIG archive
    List { big: PathBuf },
    /// Extract a single entry to stdout (binary)
    Extract { big: PathBuf, entry: String },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::List { big } => {
            let entries = big_unpack::list_entries(&big)?;
            for e in entries {
                println!("{}", e);
            }
        }
        Cmd::Extract { big, entry } => {
            let bytes = big_unpack::extract_entry(&big, &entry)?;
            use std::io::Write;
            let mut out = std::io::stdout();
            out.write_all(&bytes)?;
        }
    }
    Ok(())
}
