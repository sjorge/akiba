//! `akiba write-nfo`: write `tvshow.nfo`, the poster and the episode NFOs.

use std::cell::OnceCell;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use akiba_core::anime::AnimeId;
use akiba_core::args::{number_arg, path_arg};
use akiba_core::cache::paths::aid_file;
use akiba_core::cache::CacheFile;
use akiba_core::config::{Command as ConfigCommand, Config, ConfigLoadError};
use akiba_core::http::{UreqClient, DEFAULT_TIMEOUT};
use akiba_core::idmap::{AniList, IdMapProvider, ListMap, LocalMap, Tmdb};
use akiba_core::logger::{self, Level};
use akiba_core::nfo::anime::{episodes, tv_show, EpisodeFileInfo};
use akiba_core::nfo::{EpisodeNfo, TvShowNfo};
use akiba_core::resolver::{Resolver, ResolverError};
use akiba_core::{PROG, VERSION};
use anidb_http::{fetch_poster, AniDbShow, Error as HttpError, HttpApi, TlsError, UreqTransport};
use clap::Args as ClapArgs;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Load(#[from] ConfigLoadError),
    #[error("Failed to list the episodes")]
    Episodes(#[source] ResolverError),
}

/// Why the metadata could not be retrieved, as the message to show.
#[derive(Debug, thiserror::Error)]
enum MetadataError {
    #[error("Please register a HTTP client on anidb and run 'configure' command again!")]
    ClientInvalid,
    #[error("Please try again in 24h, we are currently banned!")]
    Banned,
    #[error("Anime with ID {0} is missing or invalid!")]
    NotFound(i64),
    #[error("Could not retrieve metadata! {0}")]
    Other(String),
}

#[derive(ClapArgs)]
pub struct Args {
    /// path to anime show directory
    #[arg(value_name = "path", value_parser = path_arg)]
    path: PathBuf,

    /// specify the anidb anime id instead of trying to auto detect
    #[arg(long, value_name = "id", value_parser = number_arg)]
    aid: Option<i64>,

    /// specify the anilist id instead of trying to auto detect
    #[arg(long, value_name = "id", value_parser = number_arg)]
    anilistid: Option<i64>,

    /// specify the themoviedb series id instead of trying to auto detect (movie ids are not supported)
    #[arg(long, value_name = "id", value_parser = number_arg)]
    tmdbid: Option<i64>,

    /// force retrieve metadata
    #[arg(long)]
    fresh: bool,

    /// overwrite existing NFO files
    #[arg(long)]
    force: bool,
}

fn plain_id(anidb: i64) -> AnimeId {
    AnimeId {
        anidb,
        anilist: None,
        tvdb: None,
        tvdb_season: None,
        tmdb: None,
        tmdb_season: None,
    }
}

fn handle_error(message: &str, error: &dyn Display) -> u8 {
    logger::log(&format!("{message}: {error}"), Level::Error);
    1
}

fn is_set(value: Option<&String>) -> bool {
    value.is_some_and(|v| !v.is_empty())
}

/// The transport of this run for the HTTP API and the poster. It is built on
/// first use, so a run that needs neither never reports a bad `tls.ca_file`.
fn shared_transport<'a>(
    config: &Config,
    cell: &'a OnceCell<UreqTransport>,
) -> Result<&'a UreqTransport, TlsError> {
    if let Some(transport) = cell.get() {
        return Ok(transport);
    }
    let transport = UreqTransport::new(DEFAULT_TIMEOUT, &config.tls_options())?;
    Ok(cell.get_or_init(|| transport))
}

/// Reads the show from the aid cache when it is fresh, otherwise asks `AniDB`
/// and rewrites the cache.
fn get_metadata(
    config: &Config,
    transport: &OnceCell<UreqTransport>,
    id: &AnimeId,
    refresh: bool,
) -> Result<AniDbShow, MetadataError> {
    let cache: CacheFile<AniDbShow> = CacheFile::new(
        aid_file(&config.cache.path, id.anidb),
        Some(config.cache.metadata_age),
    );
    if !refresh {
        // a cache that cannot be read is fetched again
        if let Ok(Some(show)) = cache.load() {
            return Ok(show);
        }
    }

    let http = &config.anidb.http_client;
    let transport =
        shared_transport(config, transport).map_err(|e| MetadataError::Other(e.to_string()))?;
    let api = HttpApi::with_transport(
        &http.url,
        http.name.as_deref().unwrap_or_default(),
        &http.version.unwrap_or_default().to_string(),
        &format!("{PROG}/{VERSION}"),
        transport,
    );
    let fetched = u32::try_from(id.anidb)
        .map_err(|_| HttpError::AnimeNotFound)
        .and_then(|aid| api.anime(aid));
    match fetched {
        Ok(show) => {
            if let Err(err) = cache.store(&show) {
                logger::log(&format!("Could not cache the metadata: {err}"), Level::Warn);
            }
            Ok(show)
        }
        Err(HttpError::ClientInvalid) => Err(MetadataError::ClientInvalid),
        Err(HttpError::Banned) => Err(MetadataError::Banned),
        Err(HttpError::AnimeNotFound) => Err(MetadataError::NotFound(id.anidb)),
        Err(other) => Err(MetadataError::Other(other.to_string())),
    }
}

/// Runs the command and returns the process exit code.
pub fn run(args: &Args) -> ExitCode {
    logger::exit_code(write_nfo(args))
}

// NOTE: one straight flow
#[allow(clippy::too_many_lines)]
fn write_nfo(args: &Args) -> Result<u8, Error> {
    let mut config = Config::load().map_err(ConfigLoadError)?;
    if let Err(problems) = config.validate(ConfigCommand::WriteNfo) {
        logger::log_problems(&problems);
        return Ok(1);
    }
    if args.force {
        config.overwrite_nfo = true;
    }

    logger::banner();
    let anime_path = &args.path;
    if !anime_path.is_dir() {
        logger::log(
            &format!("Directory \"{}\" does not exist!", anime_path.display()),
            Level::Error,
        );
        return Ok(1);
    }
    let mut title = anime_path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());

    let mut resolver = Resolver::new(&config);
    if let Err(err) = resolver.refresh(false) {
        return Ok(handle_error("Failed to initialize anime resolver", &err));
    }

    // NOTE: a failing local or list mapping only loses ids, so it is a warning
    let mut local = LocalMap::new(&config);
    if let Err(err) = local.refresh() {
        logger::log(
            &format!("Failed to initialize anime local mapper: {err}"),
            Level::Warn,
        );
    }
    let http = match UreqClient::new(DEFAULT_TIMEOUT, &config.tls_options()) {
        Ok(http) => http,
        Err(err) => return Ok(handle_error("Failed to initialize the HTTP client", &err)),
    };
    let mut list = ListMap::new(&config);
    if let Err(err) = list.refresh(&http) {
        logger::log(
            &format!("Failed to initialize anime list mapper: {err}"),
            Level::Warn,
        );
    }

    let mut anilist = None;
    if is_set(config.anilist.token.as_ref()) {
        match AniList::new(&http, config.anilist.token.as_deref()) {
            Ok(provider) => anilist = Some(provider),
            Err(err) => {
                return Ok(handle_error(
                    "Failed to initialize anime remote mappers",
                    &err,
                ))
            }
        }
    }
    let mut tmdb = None;
    if is_set(config.tmdb.api_key.as_ref()) {
        match Tmdb::new(&http, config.tmdb.api_key.as_deref()) {
            Ok(provider) => tmdb = Some(provider),
            Err(err) => {
                return Ok(handle_error(
                    "Failed to initialize anime remote mappers",
                    &err,
                ))
            }
        }
    }

    logger::log_anime(&format!("{title}: Identifying ..."), Level::Step, None);
    let found = match args.aid.filter(|&aid| aid != 0) {
        Some(aid) => Some(plain_id(aid)),
        None => resolver.id(&title),
    };
    // unable to identify anime, exit
    let Some(mut id) = found else {
        logger::log_anime(
            &format!("{title}: Failed to identify anime!"),
            Level::Error,
            None,
        );
        return Ok(1);
    };

    // normalize title
    for variant in resolver.titles(&id) {
        if variant.kind.as_deref() == Some("main") && variant.language == "x-jat" {
            title.clone_from(&variant.title);
        }
    }
    logger::log_anime(&format!("{title}: Identifying ..."), Level::Done, Some(&id));

    // add additional ids from cli flags
    let step = |message: &str, id: &AnimeId| {
        logger::log_anime(&format!("{title}: {message} ..."), Level::Step, Some(id));
    };
    let done = |message: &str, id: &AnimeId| {
        logger::log_anime(&format!("{title}: {message} ..."), Level::Done, Some(id));
    };
    step("Mapping IDs from cli flags", &id);
    if let Some(anilist_id) = args.anilistid.filter(|&v| v != 0) {
        id.anilist = Some(anilist_id);
    }
    if let Some(tmdb_id) = args.tmdbid.filter(|&v| v != 0) {
        id.tmdb = Some(tmdb_id);
        id.tmdb_season = Some(1); // hardcode this for now
    }
    done("Mapping IDs from cli flags", &id);

    // add additional ids from local and online lists, then the remote
    // providers for what is still unknown. Nothing is overwritten.
    let titles = resolver.titles(&id).to_vec();
    let apply = |provider: &mut dyn IdMapProvider, id: &mut AnimeId| {
        if let Err(err) = provider.apply(id, &titles, false) {
            logger::log_anime(
                &format!("{title}: {} mapping failed: {err}", provider.name()),
                Level::Warn,
                Some(id),
            );
        }
    };

    step("Mapping IDs from local mapping", &id);
    apply(&mut local, &mut id);
    done("Mapping IDs from local mapping", &id);

    step("Mapping IDs from list mapping", &id);
    apply(&mut list, &mut id);
    done("Mapping IDs from list mapping", &id);

    if let Some(provider) = anilist.as_mut().filter(|_| id.anilist.is_none()) {
        step("Looking up Anilist ID", &id);
        apply(provider, &mut id);
        done("Looking up Anilist ID", &id);
    }
    if let Some(provider) = tmdb.as_mut().filter(|_| id.tmdb.is_none()) {
        step("Looking up Tmdb ID", &id);
        apply(provider, &mut id);
        done("Looking up Tmdb ID", &id);
    }

    // lookup metadata
    let transport = OnceCell::new();
    step("Retrieving metadata", &id);
    let metadata = match get_metadata(&config, &transport, &id, args.fresh) {
        Ok(metadata) => metadata,
        Err(err) => {
            logger::log_anime(&format!("{title}: {err}"), Level::Error, Some(&id));
            return Ok(1);
        }
    };
    done("Retrieving metadata", &id);

    // write tvshow.nfo
    step("Writing show NFO", &id);
    if let Err(err) = write_show(&config, &transport, &id, &metadata, anime_path) {
        logger::log_anime(
            &format!("{title}: Failed to write show NFO ..."),
            Level::Error,
            Some(&id),
        );
        if let Some(reason) = err {
            logger::log(&reason, Level::Error);
        }
        return Ok(1);
    }
    done("Writing show NFO", &id);

    // write episode.nfo
    let mut code = 0;
    let files = Resolver::episodes(anime_path).map_err(Error::Episodes)?;
    for file in files {
        let mut suffix = file.episode_start.clone();
        if file.episode_end != file.episode_start {
            suffix = format!("{suffix}-{}", file.episode_end);
        }

        step(&format!("Writing episode {suffix} NFO"), &id);
        let info = EpisodeFileInfo {
            episode_start: &file.episode_start,
            episode_end: &file.episode_end,
            title: &file.title,
        };
        let written = EpisodeNfo::new(episodes(&info, &metadata), &file.path)
            .and_then(|mut nfo| nfo.write(config.overwrite_nfo));
        if written.is_ok() {
            done(&format!("Writing episode {suffix} NFO"), &id);
        } else {
            logger::log_anime(
                &format!("{title}: Failed to write episode {suffix} NFO ..."),
                Level::Error,
                Some(&id),
            );
            code = 1;
        }
    }
    Ok(code)
}

/// Writes `tvshow.nfo` and the poster. The error is the reason to show, when
/// there is one.
fn write_show(
    config: &Config,
    transport: &OnceCell<UreqTransport>,
    id: &AnimeId,
    metadata: &AniDbShow,
    dir: &Path,
) -> std::result::Result<(), Option<String>> {
    let show = tv_show(id, metadata).map_err(|e| Some(e.to_string()))?;
    let mut nfo = TvShowNfo::new(show, dir).map_err(|e| Some(e.to_string()))?;
    if !nfo.is_valid() {
        return Err(None);
    }

    // NOTE: the poster is only fetched when it is wanted and will be stored,
    // and with the option off it is never touched, even with --force
    let overwrite = config.overwrite_nfo;
    let mut poster = None;
    if config.anidb.poster && !metadata.picture.is_empty() && nfo.needs_poster(overwrite) {
        let fetched = shared_transport(config, transport)
            .map_err(anidb_http::Error::from)
            .and_then(|transport| fetch_poster(transport, &metadata.picture));
        match fetched {
            Ok(bytes) => poster = Some(bytes),
            Err(err) => {
                logger::log(&format!("Could not fetch the poster: {err}"), Level::Warn);
            }
        }
    }
    nfo.write(overwrite, poster.as_deref())
        .map_err(|e| Some(e.to_string()))
}

#[cfg(test)]
mod tests {
    use akiba_core::config::Config;
    use akiba_test_support::{Env, Outcome};

    use super::*;

    fn write_nfo(env: &Env, args: &[&str]) -> Outcome {
        env.run::<Args>(args, run)
    }

    /// Writes the config the command needs: the HTTP client, no poster.
    fn configure() {
        let mut config = Config::load().expect("config");
        config.anidb.http_client.name = Some("http".to_owned());
        config.anidb.http_client.version = Some(1);
        config.anidb.poster = false;
        config.save().expect("save config");
    }

    #[test]
    fn fails_for_a_missing_directory_and_missing_configuration() {
        let env = Env::new();
        let out = write_nfo(&env, &["/nope"]);
        assert_eq!(out.code, 1);
        assert!(out.stdout.contains("AniDB HTTP Client not configured!"));

        configure();
        let out = write_nfo(&env, &["/nope"]);
        assert_eq!(out.code, 1);
        assert!(out.stdout.contains("Directory \"/nope\" does not exist!"));
    }

    #[test]
    fn rejects_bad_ids() {
        let env = Env::new();
        assert_eq!(write_nfo(&env, &["/tmp", "--aid", "abc"]).code, 1);
    }
}
