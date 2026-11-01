//! `akiba rename`: rename episodes by their `AniDB` record.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use akiba_core::anime::is_episode_file;
use akiba_core::args::{dir_arg, format_arg, path_arg, resolve};
use akiba_core::config::{Command as ConfigCommand, Config, ConfigLoadError};
use akiba_core::logger::{self, Level};
use akiba_core::renamer::{
    Action, AnidbError, Episode, Outcome, Renamer, RenamerError, RenamerOptions,
};
use akiba_core::text::natural_cmp;
use anidb_udp::mylist::MylistState;
use anidb_udp::ResponseCode;
use clap::{Args as ClapArgs, ValueEnum};

const IGNORE_EXTENSIONS: [&str; 3] = ["nfo", "jpg", "png"];
const IGNORE_FILES: [&str; 2] = [".plexmatch", ".ignore"];

#[derive(Clone, Copy, ValueEnum)]
#[allow(clippy::enum_variant_names)]
enum MylistChoice {
    #[value(name = "internal_storage")]
    InternalStorage,
    #[value(name = "external_storage")]
    ExternalStorage,
    #[value(name = "remote_storage")]
    RemoteStorage,
}

impl MylistChoice {
    fn state(self) -> MylistState {
        match self {
            Self::InternalStorage => MylistState::InternalStorage,
            Self::ExternalStorage => MylistState::ExternalStorage,
            Self::RemoteStorage => MylistState::RemoteStorage,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::InternalStorage => "internal_storage",
            Self::ExternalStorage => "external_storage",
            Self::RemoteStorage => "remote_storage",
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(ClapArgs)]
pub struct Args {
    /// path to anime show directory or episode file
    #[arg(value_name = "path", value_parser = path_arg)]
    path: PathBuf,

    /// force rehash of already hashed
    #[arg(long)]
    rehash: bool,

    /// force retrieve metadata
    #[arg(long)]
    fresh: bool,

    /// overwrite if destination exists
    #[arg(long)]
    force: bool,

    /// copy file instead of move
    #[arg(long)]
    copy: bool,

    /// create symlink at old source path to new destination path
    #[arg(long, conflicts_with = "copy")]
    symlink: bool,

    /// update mylist state for identified files
    #[arg(long, value_name = "state", value_enum)]
    mylist_state: Option<MylistChoice>,

    /// do not rename the episodes
    #[arg(long)]
    dry_run: bool,

    /// overwrite format for the desination filename
    #[arg(long, value_name = "format", value_parser = format_arg)]
    format: Option<String>,

    /// overwrite desination path
    #[arg(long, value_name = "path", value_parser = dir_arg)]
    target_path: Option<PathBuf>,

    /// only print ed2klinks
    #[arg(
        long,
        conflicts_with_all = [
            "force", "copy", "symlink", "mylist_state", "format", "target_path", "dry_run"
        ]
    )]
    print_ed2klinks: bool,
}

/// Text for a failed `AniDB` UDP request, bans and outages already say how long
/// to wait, the other errors get the generic hint.
fn udp_error_text(error: &AnidbError) -> String {
    let waits_already = error.code().is_some_and(ResponseCode::asks_to_wait);
    if waits_already {
        error.to_string()
    } else {
        format!("{error}, try again in 30 minutes.")
    }
}

/// Logs the reason of a failed identify or mylist update.
fn log_failure(error: &RenamerError) {
    match error {
        RenamerError::Anidb(error) => {
            logger::log(
                &format!("AniDB UDP Client: {}", udp_error_text(error)),
                Level::Error,
            );
        }
        other => logger::log(&other.to_string(), Level::Error),
    }
}

/// Whether a failed step ends the run. Only the `AniDB` errors of
/// [`AnidbError::is_session_fatal`] do, a failure of one file never stops the others.
fn stops_run(error: &RenamerError) -> bool {
    matches!(error, RenamerError::Anidb(error) if error.is_session_fatal())
}

/// Collects the episode files below `dir`. Symlinks and the ignored files are
/// skipped, and only the known episode extensions are kept.
fn walk(dir: &Path, episodes: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let name = entry?.file_name();
        let name_text = name.to_string_lossy();
        let path = dir.join(&name);

        if IGNORE_FILES.contains(&name_text.as_ref()) {
            continue;
        }
        if path
            .extension()
            .is_some_and(|ext| IGNORE_EXTENSIONS.iter().any(|known| ext == *known))
        {
            continue;
        }

        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            // NOTE: a directory named like an episode is not listed, since it
            // can never be identified
            walk(&path, episodes)?;
            continue;
        }
        if is_episode_file(&path) {
            episodes.push(path);
        }
    }
    Ok(())
}

fn base_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Runs the command and returns the process exit code.
pub fn run(args: &Args) -> ExitCode {
    logger::exit_code(rename(args))
}

fn rename(args: &Args) -> Result<u8, ConfigLoadError> {
    let config = Config::load().map_err(ConfigLoadError)?;
    if let Err(problems) = config.validate(ConfigCommand::Renamer) {
        logger::log_problems(&problems);
        return Ok(1);
    }
    // NOTE: the options reach the renamer through its options, not the config
    let options = RenamerOptions {
        target: args.target_path.clone(),
        rehash: args.rehash,
        format: args.format.clone(),
        overwrite: args.force,
    };

    if !args.print_ed2klinks {
        logger::banner();
    }
    let mut renamer = match Renamer::new(&config, options) {
        Ok(renamer) => renamer,
        Err(err) => {
            logger::log(&err.to_string(), Level::Error);
            return Ok(1);
        }
    };

    let code = process(args, &config, &mut renamer);
    renamer.destroy();
    Ok(code)
}

fn process(args: &Args, config: &Config, renamer: &mut Renamer) -> u8 {
    let anime_path = &args.path;
    if !anime_path.exists() {
        logger::log(
            &format!(
                "Directory or file \"{}\" does not exist!",
                anime_path.display()
            ),
            Level::Error,
        );
        return 1;
    }

    let mut episode_files = Vec::new();
    if anime_path.is_dir() {
        if let Err(err) = walk(anime_path, &mut episode_files) {
            logger::log(
                &format!("Could not read \"{}\": {err}", anime_path.display()),
                Level::Error,
            );
            return 1;
        }
    } else {
        episode_files.push(anime_path.clone());
    }

    if !args.print_ed2klinks {
        let target = args
            .target_path
            .clone()
            .or_else(|| config.renamer.target_path.clone())
            .unwrap_or_else(|| resolve(Path::new(".")));
        logger::log(&format!("Target Path: {}", target.display()), Level::Info);
        logger::log(
            &format!(
                "Format: {}",
                args.format.as_deref().unwrap_or(&config.renamer.format)
            ),
            Level::Info,
        );
        if episode_files.is_empty() {
            logger::log("Nothing to rename ...", Level::Done);
        } else {
            logger::log("Renaming files ...", Level::Info);
        }
    }

    episode_files.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));

    let mut code = 0;
    let total = episode_files.len();
    for (index, episode_file) in episode_files.iter().enumerate() {
        let files_left = total - index - 1;
        let name = base_name(episode_file);
        let identifying = format!("{name}: Identifying ...");
        if !args.print_ed2klinks {
            logger::log(&identifying, Level::Step);
        }

        let mut episode =
            match renamer.identify_with(episode_file, args.fresh, args.print_ed2klinks) {
                Ok(episode) => episode,
                Err(err) => {
                    logger::log(&identifying, Level::Error);
                    log_failure(&err);
                    code = 1;
                    if stops_run(&err) {
                        stopped(files_left);
                        return 1;
                    }
                    continue;
                }
            };

        if args.print_ed2klinks {
            // raw output, so the links can be piped
            let _ = writeln!(io::stdout(), "{}", episode.ed2k.link);
        } else if episode.data.is_some() {
            logger::log(&identifying, Level::Done);
            if let Some(choice) = args.mylist_state.filter(|_| !args.dry_run) {
                match update_mylist(renamer, &episode, &name, choice) {
                    Ok(true) => {}
                    // AniDB did not confirm: logged as an error, the file is still renamed
                    Ok(false) => code = 1,
                    Err(err) => {
                        log_failure(&err);
                        code = 1;
                        if stops_run(&err) {
                            stopped(files_left);
                            return 1;
                        }
                        // the file failed, so it is not renamed either
                        continue;
                    }
                }
            }
            code |= rename_one(renamer, &mut episode, &name, args);
        } else {
            logger::log(&identifying, Level::Error);
            code = 1;
        }
    }
    code
}

fn stopped(files_left: usize) {
    if files_left > 0 {
        logger::log(
            &format!("Stopped, {files_left} file(s) not processed."),
            Level::Warn,
        );
    }
}

/// Updates the mylist state. `Ok(false)` means `AniDB` did not confirm it.
fn update_mylist(
    renamer: &mut Renamer,
    episode: &Episode,
    name: &str,
    choice: MylistChoice,
) -> Result<bool, RenamerError> {
    let message = format!("{name}: Updating mylist state to {} ...", choice.name());
    logger::log(&message, Level::Step);
    match renamer.set_mylist_state(episode, choice.state()) {
        Ok(confirmed) => {
            logger::log(&message, if confirmed { Level::Done } else { Level::Error });
            Ok(confirmed)
        }
        Err(err) => {
            logger::log(&message, Level::Error);
            Err(err)
        }
    }
}

/// Renames one identified episode and reports it. Returns the exit code.
fn rename_one(renamer: &mut Renamer, episode: &mut Episode, name: &str, args: &Args) -> u8 {
    logger::log(&format!("{name}: Renaming ..."), Level::Step);
    let action = if args.symlink {
        Action::Symlink
    } else if args.copy {
        Action::Copy
    } else {
        Action::Move
    };
    if let Err(err) = renamer.rename(episode, action, args.dry_run) {
        logger::log(&format!("{name}: Renaming ..."), Level::Error);
        logger::log(&err.to_string(), Level::Error);
        return 1;
    }

    let Some(destination) = &episode.destination_path else {
        logger::log(&format!("{name}: Renaming ..."), Level::Error);
        return 0;
    };
    let message = match episode.action {
        Some(Outcome::UpToDate) => "Already correctly named.".to_owned(),
        outcome => placed_message(outcome, destination),
    };
    logger::log(&format!("{name}: {message}"), Level::Done);

    // the sidecars follow the episode, a failure there leaves the episode renamed
    let mut code = 0;
    for sidecar in &episode.sidecars {
        let sidecar_name = base_name(&sidecar.source);
        if let Some(error) = &sidecar.error {
            logger::log(&format!("{sidecar_name}: Renaming ..."), Level::Error);
            logger::log(error, Level::Error);
            code = 1;
        } else {
            let message = placed_message(Some(sidecar.action), &sidecar.destination);
            logger::log(&format!("{sidecar_name}: {message}"), Level::Done);
        }
    }
    code
}

fn placed_message(outcome: Option<Outcome>, destination: &Path) -> String {
    let verb = if outcome == Some(Outcome::Copy) {
        "Copied"
    } else {
        "Moved"
    };
    let linked = if outcome == Some(Outcome::Symlink) {
        " and symlinked"
    } else {
        ""
    };
    format!("{verb} to {}{linked}!", destination.display())
}

#[cfg(test)]
mod tests;
