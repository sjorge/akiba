//! `akiba configure`: update the configuration file.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use akiba_core::args::{dir_arg, format_arg, resolve};
use akiba_core::config::{Command as ConfigCommand, Config, ConfigLoadError};
use akiba_core::logger::{self, Level};
use akiba_core::num::parse_i64;
use akiba_core::paths;
use anidb_http::TlsOptions;
use clap::Args as ClapArgs;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Load(#[from] ConfigLoadError),
    #[error("Failed to dump the configuration")]
    Dump(#[source] serde_json::Error),
}

// NOTE: every `--no-*` negation is its own flag
#[allow(clippy::struct_excessive_bools)]
#[derive(ClapArgs)]
pub struct Args {
    #[arg(
        long,
        value_name = "http_client",
        value_parser = client_arg,
        help = "your anidb http client (format: <name>/<version>)"
    )]
    anidb_http_client: Option<String>,

    #[arg(
        long,
        value_name = "udp_client",
        value_parser = udp_client_arg,
        help = "your anidb udp client (format: <name>/<version>, name: 4 to 16 lower case letters a-z)"
    )]
    anidb_udp_client: Option<String>,

    /// your anidb username
    #[arg(long, value_name = "username")]
    anidb_auth_username: Option<String>,

    /// your anidb password
    #[arg(long, value_name = "password")]
    anidb_auth_password: Option<String>,

    /// enable anidb encryption with your udp api key
    #[arg(long, value_name = "key", overrides_with = "no_anidb_auth_api_key")]
    anidb_auth_api_key: Option<String>,

    /// disable anidb encryption
    #[arg(long, overrides_with = "anidb_auth_api_key")]
    no_anidb_auth_api_key: bool,

    /// enable anidb poster fetching
    #[arg(long, overrides_with = "no_anidb_poster")]
    anidb_poster: bool,

    /// disable anidb poster fetching
    #[arg(long, overrides_with = "anidb_poster")]
    no_anidb_poster: bool,

    /// your anilist http client token
    #[arg(long, value_name = "token")]
    anilist_token: Option<String>,

    /// your tmdb API key
    #[arg(long, value_name = "key")]
    tmdb_api_key: Option<String>,

    /// overwrite existing nfo by default
    #[arg(long, overrides_with = "no_overwrite_nfo")]
    overwrite_nfo: bool,

    /// keep existing nfo by default
    #[arg(long, overrides_with = "overwrite_nfo")]
    no_overwrite_nfo: bool,

    /// trust only the CA certificates in this PEM file (PEM format)
    #[arg(
        long,
        value_name = "path",
        value_parser = ca_file_arg,
        overrides_with = "no_tls_ca_file"
    )]
    tls_ca_file: Option<PathBuf>,

    /// use the system certificate store again, found automatically
    #[arg(long, overrides_with = "tls_ca_file")]
    no_tls_ca_file: bool,

    /// format for the desination filename
    #[arg(long, value_name = "format", value_parser = format_arg)]
    rename_format: Option<String>,

    /// desination path
    #[arg(
        long,
        value_name = "path",
        value_parser = dir_arg,
        overrides_with = "no_rename_target_path"
    )]
    rename_target_path: Option<PathBuf>,

    /// use current working directory
    #[arg(long, overrides_with = "rename_target_path")]
    no_rename_target_path: bool,

    /// dump configuration
    #[arg(long)]
    dump: bool,
}

/// Splits `name/version`. A missing version is 1. The version must be a whole
/// number that fits the config's unsigned integer (`name/1x` is rejected).
fn split_client(value: &str) -> Option<(String, u32)> {
    let mut parts = value.split('/');
    let name = parts.next().unwrap_or_default();
    let version = parts.next().unwrap_or("1");
    let version = u32::try_from(parse_i64(version)?).ok()?;
    Some((name.to_owned(), version))
}

fn client_arg(value: &str) -> Result<String, String> {
    split_client(value).ok_or_else(|| "Expecting a number for version.".to_owned())?;
    Ok(value.to_owned())
}

/// The UDP client name follows the spec's rule, the HTTP one is not checked.
fn udp_client_arg(value: &str) -> Result<String, String> {
    let name = value.split('/').next().unwrap_or_default();
    if !anidb_udp::is_valid_client_name(name) {
        return Err("Expecting 4 to 16 lower case letters (a-z) for the client name.".to_owned());
    }
    client_arg(value)
}

/// The CA file must exist and hold at least one PEM certificate. The empty
/// string passes through and, as for the other options, changes nothing.
fn ca_file_arg(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() {
        return Ok(PathBuf::new());
    }
    let path = resolve(Path::new(value));
    TlsOptions::new(Some(path.clone()))
        .validate()
        .map_err(|e| e.to_string())?;
    Ok(path)
}

/// A set option that is not the empty string, so an empty value counts as unset.
fn given(value: Option<&String>) -> Option<&String> {
    value.filter(|v| !v.is_empty())
}

/// Runs the command and returns the process exit code.
pub fn run(args: &Args) -> ExitCode {
    logger::exit_code(configure(args))
}

fn configure(args: &Args) -> Result<u8, Error> {
    let mut config = Config::load().map_err(ConfigLoadError)?;

    if let Some((name, version)) =
        given(args.anidb_http_client.as_ref()).and_then(|c| split_client(c))
    {
        config.anidb.http_client.name = Some(name);
        config.anidb.http_client.version = Some(version);
    }
    if let Some((name, version)) =
        given(args.anidb_udp_client.as_ref()).and_then(|c| split_client(c))
    {
        config.anidb.udp_client.name = Some(name);
        config.anidb.udp_client.version = Some(version);
    }

    if let Some(username) = given(args.anidb_auth_username.as_ref()) {
        config.anidb.udp_client.username = Some(username.clone());
    }
    if let Some(password) = given(args.anidb_auth_password.as_ref()) {
        config.anidb.udp_client.password = Some(password.clone());
    }

    if args.no_anidb_auth_api_key {
        config.anidb.udp_client.api_key = None;
    } else if let Some(key) = given(args.anidb_auth_api_key.as_ref()) {
        config.anidb.udp_client.api_key = Some(key.clone());
    }

    if args.no_anidb_poster {
        config.anidb.poster = false;
    } else if args.anidb_poster {
        config.anidb.poster = true;
    }

    if let Some(key) = given(args.tmdb_api_key.as_ref()) {
        config.tmdb.api_key = Some(key.clone());
    }
    if let Some(token) = given(args.anilist_token.as_ref()) {
        config.anilist.token = Some(token.clone());
    }

    if args.no_overwrite_nfo {
        config.overwrite_nfo = false;
    } else if args.overwrite_nfo {
        config.overwrite_nfo = true;
    }

    if args.no_tls_ca_file {
        config.tls.ca_file = None;
    } else if let Some(path) = &args.tls_ca_file {
        // like the other options, an empty value changes nothing
        if !path.as_os_str().is_empty() {
            config.tls.ca_file = Some(path.clone());
        }
    }

    if let Some(format) = given(args.rename_format.as_ref()) {
        config.renamer.format.clone_from(format);
    }
    if args.no_rename_target_path {
        config.renamer.target_path = None;
    } else if let Some(path) = &args.rename_target_path {
        // the parser already normalised it: absolute, without a trailing slash
        config.renamer.target_path = Some(path.clone());
    }

    let mut code = 0;
    if config.save().is_err() {
        let file = paths::config_file().map_or_else(
            |_| "the config file".to_owned(),
            |f| f.display().to_string(),
        );
        logger::log(&format!("Failed to update {file}!"), Level::Error);
        code = 1;
    } else if let Err(problems) = config.validate(ConfigCommand::WriteNfo) {
        logger::log_problems(&problems);
        code = 1;
    } else if let Err(problems) = config.validate(ConfigCommand::Renamer) {
        logger::log_problems(&problems);
        code = 1;
    }

    if args.dump {
        logger::banner();
        // NOTE: the dump is intentionally the full config, secrets included.
        // It is data, not a log line, so it goes to stdout as it is.
        let json = serde_json::to_string_pretty(&config).map_err(Error::Dump)?;
        let _ = writeln!(std::io::stdout(), "{json}");
    }
    Ok(code)
}

#[cfg(test)]
mod tests;
