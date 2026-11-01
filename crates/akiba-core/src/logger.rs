//! User facing output: `[II]`-style prefixes, colour on a terminal, the id
//! section, the banner, and a capture hook for tests.

use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::anime::AnimeId;
use crate::config::ConfigProblem;
use crate::{PROG, VERSION};

/// Widest banner rule; a narrower terminal shortens it so it does not wrap.
const BANNER_WIDTH: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
    Step,
    Done,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stream {
    Stdout,
    Stderr,
}

/// Where output goes when the capture hook is active.
struct Sinks {
    out: Box<dyn Write + Send>,
    err: Box<dyn Write + Send>,
    colour: bool,
}

/// Global logger state. `sinks` is `None` for the real stdout and stderr.
struct State {
    sinks: Option<Sinks>,
    banner_printed: bool,
}

static STATE: Mutex<State> = Mutex::new(State {
    sinks: None,
    banner_printed: false,
});

/// Serialises `capture` calls, so captured output never mixes between tests.
static CAPTURE_LOCK: Mutex<()> = Mutex::new(());

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// True when output goes to a terminal. A capture decides this itself.
fn use_colour() -> bool {
    match &state().sinks {
        Some(sinks) => sinks.colour,
        None => io::stdout().is_terminal(),
    }
}

fn emit(stream: Stream, text: &str) {
    // NOTE: flush after every write. A coloured step line has no newline, and
    // Rust's stdout is line buffered, so without the flush the `>>` line would
    // only show up together with the line that replaces it.
    let mut state = state();
    match (&mut state.sinks, stream) {
        (Some(sinks), Stream::Stdout) => {
            let _ = sinks.out.write_all(text.as_bytes());
            let _ = sinks.out.flush();
        }
        (Some(sinks), Stream::Stderr) => {
            let _ = sinks.err.write_all(text.as_bytes());
            let _ = sinks.err.flush();
        }
        (None, Stream::Stdout) => {
            let mut out = io::stdout().lock();
            let _ = out.write_all(text.as_bytes());
            let _ = out.flush();
        }
        (None, Stream::Stderr) => {
            let mut err = io::stderr().lock();
            let _ = err.write_all(text.as_bytes());
            let _ = err.flush();
        }
    }
}

/// Tag text and ANSI colour code for each level.
fn label(level: Level) -> (&'static str, &'static str) {
    match level {
        Level::Error => ("!!", "31"),
        Level::Warn => ("WW", "33"),
        Level::Step => (">>", "33"),
        Level::Done => ("OK", "32"),
        Level::Info => ("II", "34"),
    }
}

/// Renders one line and picks its stream. Errors go to stderr only when coloured.
/// A coloured step has no newline, so the next line can overwrite it.
fn render(level: Level, colour: bool, ids: &str, msg: &str) -> (Stream, String) {
    let (tag, code) = label(level);
    let (prefix, end) = if colour {
        let end = if level == Level::Step { "" } else { "\n" };
        (format!("\x1b[2K\r[\x1b[{code}m{tag}\x1b[0m] "), end)
    } else {
        (format!("[{tag}] "), "\n")
    };
    let stream = if colour && level == Level::Error {
        Stream::Stderr
    } else {
        Stream::Stdout
    };
    (stream, format!("{prefix}{ids}{msg}{end}"))
}

/// The `{###|  1234}` section for an anime, or a blank one for `None`. A zero
/// id counts as unknown, as a missing id does.
fn id_section(id: Option<&AnimeId>, colour: bool) -> String {
    let Some(id) = id else {
        return "{   |      } ".to_owned();
    };
    let mark = |known: bool, code: &str| match (known, colour) {
        (false, _) => " ".to_owned(),
        (true, true) => format!("\x1b[{code}m#\x1b[0m"),
        (true, false) => "#".to_owned(),
    };
    let anidb = mark(id.anidb != 0, "31");
    let anilist = mark(id.anilist.is_some_and(|v| v != 0), "34");
    let tmdb = mark(id.tmdb.is_some_and(|v| v != 0), "36");
    format!("{{{anidb}{anilist}{tmdb}|{:>6}}} ", id.anidb)
}

/// Logs one line with a level prefix.
pub fn log(msg: &str, level: Level) {
    let colour = use_colour();
    let (stream, line) = render(level, colour, "", msg);
    emit(stream, &line);
}

/// Logs an error at the error level: its message, then the message of each
/// error in its `source()` chain, joined with `": "`.
pub fn log_error(error: &dyn std::error::Error) {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    log(&text, Level::Error);
}

/// Turns a command's result into the process exit code. An error is logged
/// with `log_error` and gives exit code 1.
pub fn exit_code<E: std::error::Error>(result: Result<u8, E>) -> ExitCode {
    match result {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            log_error(&err);
            ExitCode::FAILURE
        }
    }
}

/// Logs each config problem as its own error line.
pub fn log_problems(problems: &[ConfigProblem]) {
    for problem in problems {
        log(problem.message, Level::Error);
    }
}

/// Logs one line with the id section. `None` prints a blank section, .
pub fn log_anime(msg: &str, level: Level, id: Option<&AnimeId>) {
    let colour = use_colour();
    let ids = id_section(id, colour);
    let (stream, line) = render(level, colour, &ids, msg);
    emit(stream, &line);
}

/// Prints the program name and a rule once per process. Later calls do nothing.
pub fn banner() {
    {
        let mut state = state();
        if state.banner_printed {
            return;
        }
        state.banner_printed = true;
    }
    log(&format!("{PROG} v{VERSION}"), Level::Info);
    emit(Stream::Stdout, &format!("{}\n", "-".repeat(banner_width())));
}

/// Width of the banner rule: the terminal width, at most `BANNER_WIDTH`. Without
/// a terminal (piped output, or captured in tests) it is `BANNER_WIDTH`.
fn banner_width() -> usize {
    let captured = state().sinks.is_some();
    let columns = if captured || !io::stdout().is_terminal() {
        None
    } else {
        terminal_size::terminal_size().map(|(terminal_size::Width(w), _)| usize::from(w))
    };
    rule_width(columns)
}

/// `min(columns, BANNER_WIDTH)`. An unknown or zero width gives `BANNER_WIDTH`.
fn rule_width(columns: Option<usize>) -> usize {
    match columns {
        Some(columns) if columns > 0 => columns.min(BANNER_WIDTH),
        _ => BANNER_WIDTH,
    }
}

/// Output produced inside `capture`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Captured {
    pub stdout: String,
    pub stderr: String,
    /// How often stdout was flushed, to check that unfinished lines get shown.
    pub stdout_flushes: usize,
}

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>, Arc<std::sync::atomic::AtomicUsize>);

impl Buffer {
    fn take(&self) -> String {
        let bytes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

/// Puts the previous state back on drop, so a panic inside a capture does not
/// leave the logger redirected.
struct Restore(Option<State>);

impl Drop for Restore {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            *state() = previous;
        }
    }
}

/// Runs `f` with stdout and stderr captured. The banner state starts fresh, and
/// `colour` is forced, so tests do not depend on whether they run in a terminal.
pub fn capture<F: FnOnce()>(colour: bool, f: F) -> Captured {
    let _serial = CAPTURE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let stdout = Buffer::default();
    let stderr = Buffer::default();
    let fresh = State {
        sinks: Some(Sinks {
            out: Box::new(stdout.clone()),
            err: Box::new(stderr.clone()),
            colour,
        }),
        banner_printed: false,
    };
    let previous = std::mem::replace(&mut *state(), fresh);
    let restore = Restore(Some(previous));

    f();

    drop(restore);
    Captured {
        stdout: stdout.take(),
        stderr: stderr.take(),
        stdout_flushes: stdout.1.load(std::sync::atomic::Ordering::SeqCst),
    }
}

#[cfg(test)]
mod tests;
