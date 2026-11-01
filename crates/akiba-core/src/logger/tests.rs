use super::*;

fn sample_id() -> AnimeId {
    AnimeId {
        anidb: 12,
        anilist: Some(3),
        tvdb: None,
        tvdb_season: None,
        tmdb: None,
        tmdb_season: None,
    }
}

#[test]
fn every_write_is_flushed_so_a_step_shows_before_it_is_replaced() {
    // the coloured step has no newline; without a flush it would stay in the
    // line buffer until the OK line arrives
    let out = capture(true, || {
        log("hashing", Level::Step);
        log("hashing", Level::Done);
    });
    assert_eq!(out.stdout_flushes, 2, "{out:?}");
}

#[test]
fn the_banner_rule_never_wraps_and_is_at_most_80_wide() {
    assert_eq!(rule_width(None), 80);
    assert_eq!(rule_width(Some(0)), 80);
    assert_eq!(rule_width(Some(75)), 75);
    assert_eq!(rule_width(Some(80)), 80);
    assert_eq!(rule_width(Some(200)), 80);
}

#[test]
fn plain_output_without_a_tty() {
    // the banner is printed once, so the second call adds nothing
    let banners = capture(false, || {
        banner();
        banner();
    });
    assert_eq!(
        banners.stdout,
        format!("[II] akiba v{VERSION}\n{}\n", "-".repeat(80))
    );
    assert_eq!(banners.stderr, "");

    let out = capture(false, || {
        log("message", Level::Error);
        log("message", Level::Warn);
        log("message", Level::Info);
        log("message", Level::Step);
        log("message", Level::Done);
        log_anime("message", Level::Info, Some(&sample_id()));
        log_anime("message", Level::Info, None);
    });
    assert_eq!(
        out.stdout,
        [
            "[!!] message",
            "[WW] message",
            "[II] message",
            "[>>] message",
            "[OK] message",
            "[II] {## |    12} message",
            "[II] {   |      } message",
            "",
        ]
        .join("\n")
    );
    assert_eq!(out.stderr, "");
}

#[test]
fn coloured_output_sends_errors_to_stderr() {
    let out = capture(true, || {
        log("message", Level::Error);
        log("message", Level::Warn);
        log("message", Level::Step);
        log("message", Level::Done);
    });
    assert_eq!(out.stderr, "\x1b[2K\r[\x1b[31m!!\x1b[0m] message\n");
    assert_eq!(
            out.stdout,
            "\x1b[2K\r[\x1b[33mWW\x1b[0m] message\n\x1b[2K\r[\x1b[33m>>\x1b[0m] message\x1b[2K\r[\x1b[32mOK\x1b[0m] message\n"
        );
}

#[test]
fn coloured_step_has_no_newline() {
    let out = capture(true, || log("working", Level::Step));
    assert_eq!(out.stdout, "\x1b[2K\r[\x1b[33m>>\x1b[0m] working");
}

#[test]
fn coloured_id_section_marks_known_providers() {
    let id = AnimeId {
        anidb: 12,
        anilist: Some(3),
        tvdb: Some(9),
        tvdb_season: None,
        tmdb: Some(0),
        tmdb_season: None,
    };
    let out = capture(true, || log_anime("m", Level::Info, Some(&id)));
    assert_eq!(
        out.stdout,
        "\x1b[2K\r[\x1b[34mII\x1b[0m] {\x1b[31m#\x1b[0m\x1b[34m#\x1b[0m |    12} m\n"
    );
}

#[test]
fn zero_ids_count_as_unknown() {
    let id = AnimeId {
        anidb: 0,
        anilist: Some(0),
        tvdb: None,
        tvdb_season: None,
        tmdb: Some(0),
        tmdb_season: None,
    };
    let out = capture(false, || log_anime("m", Level::Info, Some(&id)));
    assert_eq!(out.stdout, "[II] {   |     0} m\n");
}
