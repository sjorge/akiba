//! The `akiba` binary: the top-level command, dispatch to the command crates
//! and exit codes.
//!
//! Exit codes are the same as in akiba 2.x: 0 on success and for
//! `--help` / `--version`, 1 for every error, including bad arguments (clap's
//! default of 2 is mapped to 1).

use std::io::Write;
use std::process::ExitCode;

use akiba_core::VERSION;
use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "akiba",
    about = "Utility to export stub-NFO's for use with jellyfin.",
    disable_version_flag = true
)]
struct Cli {
    /// output the version number
    #[arg(short = 'V', long = "version")]
    version: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// update configuration file
    Configure(akiba_cmd_configure::Args),
    /// write NFO files
    WriteNfo(akiba_cmd_write_nfo::Args),
    /// rename episodes
    Rename(akiba_cmd_rename::Args),
    /// manage local ID mapping entries
    Idmap(akiba_cmd_idmap::Args),
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let _ = err.print();
            return ExitCode::from(u8::from(err.use_stderr()));
        }
    };

    if cli.version {
        // NOTE: like commander, only the bare version number
        let _ = writeln!(std::io::stdout(), "{VERSION}");
        return ExitCode::SUCCESS;
    }

    let Some(command) = cli.command else {
        // commander prints the help to stderr and exits 1 without a command
        let _ = write!(std::io::stderr(), "{}", Cli::command().render_help());
        return ExitCode::FAILURE;
    };

    match command {
        Command::Configure(args) => akiba_cmd_configure::run(&args),
        Command::WriteNfo(args) => akiba_cmd_write_nfo::run(&args),
        Command::Rename(args) => akiba_cmd_rename::run(&args),
        Command::Idmap(args) => akiba_cmd_idmap::run(&args),
    }
}
