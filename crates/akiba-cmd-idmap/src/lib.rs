//! `akiba idmap`: manage local ID mapping entries.

use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::process::ExitCode;

use akiba_core::args::number_arg;
use akiba_core::config::{Config, ConfigLoadError};
use akiba_core::idmap::LocalMap;
use akiba_core::logger::{self, Level};
use clap::Args as ClapArgs;

#[allow(clippy::struct_excessive_bools)]
#[derive(ClapArgs)]
pub struct Args {
    /// anidb id to manage mapping for
    #[arg(value_name = "aid", value_parser = number_arg)]
    aid: i64,

    /// add maping to this anilist id
    #[arg(
        long,
        value_name = "id",
        value_parser = number_arg,
        overrides_with = "no_anilistid"
    )]
    anilistid: Option<i64>,

    /// remove mapping to anilist id
    #[arg(long, overrides_with = "anilistid")]
    no_anilistid: bool,

    /// add maping to this themoviedb sereis id (movie ids are not supported)
    #[arg(
        long,
        value_name = "id",
        value_parser = number_arg,
        overrides_with = "no_tmdbid"
    )]
    tmdbid: Option<i64>,

    /// remove mapping to themoviedb series id
    #[arg(long, overrides_with = "tmdbid")]
    no_tmdbid: bool,

    /// show mapping for aid
    #[arg(long)]
    no_show: bool,
}

/// Runs the command and returns the process exit code.
pub fn run(args: &Args) -> ExitCode {
    logger::exit_code(idmap(args))
}

fn idmap(args: &Args) -> Result<u8, ConfigLoadError> {
    let config = Config::load().map_err(ConfigLoadError)?;
    let aid = args.aid;

    logger::banner();
    logger::log(
        &format!("Using cache path: {}", config.cache.path.display()),
        Level::Info,
    );
    if !config.cache.path.is_dir()
        && DirBuilder::new()
            .recursive(true)
            .mode(0o750)
            .create(&config.cache.path)
            .is_err()
    {
        logger::log("Could not create cache path!", Level::Error);
        return Ok(1);
    }

    // a missing or empty mapping file is fine, only an unreadable one is not
    let mut local = LocalMap::new(&config);
    if local.refresh().is_err() {
        logger::log("Could not read local mappping file!", Level::Error);
        logger::log(
            "Please check permissions, problem persists delete the mapping file.",
            Level::Info,
        );
        let file = akiba_core::cache::paths::local_file(&config.cache.path);
        logger::log(&format!("Mapping File: {}", file.display()), Level::Info);
        return Ok(1);
    }

    if args.no_anilistid {
        if local.entry(aid).is_some_and(|e| e.anilist.is_some()) {
            logger::log(
                &format!("Unmapping AniList ID from AniDB ID {aid}"),
                Level::Info,
            );
            local.unset_anilist(aid);
        }
    } else if let Some(anilist) = args.anilistid {
        local.set_anilist(aid, anilist);
    }
    if args.no_tmdbid {
        if local.entry(aid).is_some_and(|e| e.tmdb.is_some()) {
            logger::log(
                &format!("Unmapping tmdbid from AniDB ID {aid}"),
                Level::Info,
            );
            local.unset_tmdb(aid);
        }
    } else if let Some(tmdb) = args.tmdbid {
        local.set_tmdb(aid, tmdb);
    }

    if local.save().is_err() {
        logger::log("Could not write local mappping file!", Level::Error);
        return Ok(1);
    }

    if !args.no_show {
        let message = match local.entry(aid) {
            Some(entry) => {
                let anilist = entry
                    .anilist
                    .filter(|&id| id != 0)
                    .map_or_else(|| "(no mapping)".to_owned(), |id| id.to_string());
                let tmdb = entry.tmdb.filter(|&id| id != 0).map_or_else(
                    || "(no mapping)".to_owned(),
                    |id| {
                        let season = entry
                            .tmdb_season
                            .map_or_else(|| "undefined".to_owned(), |s| s.to_string());
                        format!("{id}:{season}")
                    },
                );
                format!("AniDB {aid} => AniList {anilist}, TheMovieDB {tmdb}")
            }
            None => format!("AniDB {aid} has no mappings."),
        };
        logger::log(&message, Level::Info);
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use akiba_test_support::{Env, Outcome};

    use super::*;

    fn idmap(env: &Env, args: &[&str]) -> Outcome {
        env.run::<Args>(args, run)
    }

    #[test]
    fn idmap_treats_an_empty_mapping_file_as_nothing_to_clean() {
        let env = Env::new();
        fs::create_dir_all(env.cache_dir()).expect("cache dir");
        fs::write(env.cache_dir().join("local.map.toml"), "").expect("empty file");
        assert_eq!(idmap(&env, &["5", "--no-anilistid"]).code, 0);
    }

    #[test]
    fn idmap_fails_on_a_corrupt_mapping_file() {
        let env = Env::new();
        fs::create_dir_all(env.cache_dir()).expect("cache dir");
        fs::write(env.cache_dir().join("local.map.toml"), "[[[ not toml").expect("file");
        let out = idmap(&env, &["5"]);
        assert_eq!(out.code, 1);
        assert!(out.stdout.contains("Could not read local mappping file!"));
    }

    #[test]
    fn idmap_accepts_every_flag() {
        let env = Env::new();
        let out = idmap(
            &env,
            &[
                "1",
                "--anilistid",
                "2",
                "--no-anilistid",
                "--tmdbid",
                "3",
                "--no-tmdbid",
                "--no-show",
            ],
        );
        assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    }
}
