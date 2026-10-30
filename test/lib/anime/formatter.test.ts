import { expect, test } from "bun:test";

import {
  animeStringFormat,
  animeStringValidate,
  AnimeStringFormatException,
} from "lib/anime/formatter";
import { sampleFormatData } from "test/helpers";

function name(value: string): string {
  return animeStringFormat(
    "{anime_name_romaji}",
    sampleFormatData({ anime_name_romaji: value }),
  );
}

test("animeStringFormat applies tags, modifiers and array indexes", (): void => {
  const data = sampleFormatData({ anime_name_romaji: "Mixed Case" });

  // plain tags, strings and numbers
  expect(
    animeStringFormat(
      "{anime_name_romaji} - {episode} ({crc32}).{filetype}",
      data,
    ),
  ).toBe("Mixed Case - 013 (deadbeef).mkv");
  expect(animeStringFormat("{aid}", data)).toBe("100");

  // case modifiers, which are case insensitive themselves
  expect(animeStringFormat("{anime_name_romaji:upper}", data)).toBe(
    "MIXED CASE",
  );
  expect(animeStringFormat("{anime_name_romaji:LOWER}", data)).toBe(
    "mixed case",
  );
  expect(animeStringFormat("{anime_name_romaji:upper_first}", data)).toBe("M");
  expect(animeStringFormat("{anime_name_romaji:lower_first}", data)).toBe("m");

  // number strips zeros, keeps ranges, falls back for anything else
  expect(animeStringFormat("{episode:number}", data)).toBe("13");
  expect(animeStringFormat("{aid:number}", data)).toBe("100");
  for (const [episode, expected] of [
    ["01-03", "1-3"],
    ["abc", "abc"],
    ["01-x", "01-x"],
  ]) {
    expect(
      animeStringFormat("{episode:number}", sampleFormatData({ episode })),
    ).toBe(expected);
  }

  // arrays default to the first entry, an out of range index joins them
  expect(animeStringFormat("{anime_category}", data)).toBe("Action");
  expect(animeStringFormat("{anime_category:1}", data)).toBe("Comedy");
  expect(animeStringFormat("{anime_category:5}", data)).toBe("Action,Comedy");
});

test("animeStringFormat sanitizes tag values but not the template", (): void => {
  expect(name("AC/DC")).toBe("ACDC");
  expect(name("It`s")).toBe("It's");
  expect(name("a:b*c")).toBe("a_b_c");
  expect(name("a:?*b")).toBe("a_b");
  expect(name(":abc:")).toBe("abc");
  expect(name("abc:.")).toBe("abc.");
  expect(name("abc: def")).toBe("abc def");
  expect(name("A & B! (2020), ~+")).toBe("A & B! (2020), ~+");

  expect(animeStringFormat("a:b/{crc32}", sampleFormatData())).toBe(
    "a:b/deadbeef",
  );
});

test("animeStringFormat cleans up directory parts and path traversal", (): void => {
  const data = sampleFormatData();

  // a trailing period is stripped from directories, not from the basename
  expect(
    animeStringFormat(
      "{anime_name_romaji}./Season 1./{crc32}.{filetype}",
      data,
    ),
  ).toBe("Test Anime/Season 1/deadbeef.mkv");
  expect(
    animeStringFormat("{anime_name_romaji}/{anime_name_romaji}.", data),
  ).toBe("Test Anime/Test Anime.");

  // NOTE: regression for refusing destinations outside the target path
  const slash = sampleFormatData({ anime_name_romaji: "/etc" });
  expect(animeStringFormat("x{anime_name_romaji:upper_first}", slash)).toBe(
    "x",
  );
  expect(animeStringFormat("x{anime_name_romaji:lower_first}", slash)).toBe(
    "x",
  );
  expect(name("../../etc")).toBe("....etc");

  // a .. is neutralised in directory parts, but survives as the basename
  // XXX: only AnimeRenamer.rename() rejects that, see renamer.test.ts
  const dots = sampleFormatData({ anime_name_romaji: ".." });
  expect(animeStringFormat("{anime_name_romaji}/file", dots)).toBe("./file");
  expect(animeStringFormat("a/{anime_name_romaji}", dots)).toBe("a/..");
});

test("unknown tags throw and validate reports them", (): void => {
  expect((): string => animeStringFormat("{nope}", sampleFormatData())).toThrow(
    AnimeStringFormatException,
  );
  try {
    animeStringFormat("{nope}", sampleFormatData());
  } catch (_e: unknown) {
    // NOTE: regression for the swapped exception name
    expect((_e as Error).name).toBe("AnimeStringFormatException");
  }

  expect(
    animeStringValidate("{anime_name_romaji} - {episode:number}.{filetype}"),
  ).toBe(true);
  expect(animeStringValidate("{nope}")).toBe(false);
  expect((): boolean => animeStringValidate("{nope}", true)).toThrow(
    AnimeStringFormatException,
  );
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
