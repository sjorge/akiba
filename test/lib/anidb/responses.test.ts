import { describe, expect, test } from "bun:test";

import { AnidbError } from "lib/anidb/error";
import {
  ANIME_DESC_PARSER,
  CALENDAR_PARSER,
  CHARACTER_PARSER,
  CREATOR_PARSER,
  EPISODE_PARSER,
  GROUP_PARSER,
  GROUP_STATUS_PARSER,
  MYLIST_PARSER,
  RANDOM_PARSER,
  parseMylistByAnime,
  parseMylistEdited,
} from "lib/anidb/responses";

const GROUP =
  "13184|850|100|12|10|Group Name|GN|#irc|irc.example|http://g|pic|1200000000|0|1|1300000000|1400000000";

function badData(fn: () => unknown, text?: string): void {
  let caught: unknown;
  try {
    fn();
  } catch (_e) {
    caught = _e;
  }
  expect(caught).toBeInstanceOf(AnidbError);
  if (text) expect((caught as AnidbError).message).toContain(text);
}

describe("GROUP (9a)", () => {
  test("no relations gives an empty list, with or without the last field", () => {
    expect(GROUP_PARSER.parseData(GROUP).grouprelations).toEqual([]);
    expect(GROUP_PARSER.parseData(`${GROUP}|`).grouprelations).toEqual([]);
  });

  test("relations are still parsed", () => {
    expect(GROUP_PARSER.parseData(`${GROUP}|5,1'7,105`).grouprelations).toEqual(
      [
        { othergroupid: 5, relationtype: "participant_in" },
        { othergroupid: 7, relationtype: "formerly" },
      ],
    );
  });

  test("a truncated record is still bad data", () => {
    badData(() => GROUP_PARSER.parseData("13184|850"), "fields");
  });
});

describe("ANIMEDESC (9b)", () => {
  test("keeps the whole text including pipes", () => {
    const part = ANIME_DESC_PARSER.parseData("0|1|first | part<br />x|y");
    expect(part.current_part).toBe(0);
    expect(part.max_parts).toBe(1);
    // raw: the client decodes line breaks after joining the parts
    expect(part.desc).toBe("first | part<br />x|y");
  });
});

describe("date flags (9c)", () => {
  test("junk is bad data", () => {
    badData(() => CALENDAR_PARSER("1|100|abc"), "dateflags");
    badData(() =>
      GROUP_PARSER.parseData(`${GROUP.replace("|1|1300", "|zz|1300")}|`),
    );
  });

  test("valid and empty flags", () => {
    expect(CALENDAR_PARSER("1|100|17")).toEqual([
      { aid: 1, startdate: 100, dateflags: ["start_unknown_day", "ended"] },
    ]);
    expect(CALENDAR_PARSER("1|100|")[0]!.dateflags).toEqual([]);
  });
});

describe("MYLIST by anime (9d)", () => {
  const entry = "1|2|3|4|5|6|1|8|st|so|ot|0";
  test("221 gives a single entry", () => {
    expect(parseMylistByAnime(221, entry).single?.lid).toBe(1);
  });

  test("312 gives groups", () => {
    const multi = parseMylistByAnime(
      312,
      "Title|12|1|2|3|0|4|G1|1-6|G2|7-12",
    ).multi!;
    expect(multi.episodes).toBe(12);
    expect(multi.groups).toEqual([
      { group: "G1", episodes: "1-6" },
      { group: "G2", episodes: "7-12" },
    ]);
  });

  test("another code is bad data", () => {
    badData(() => parseMylistByAnime(311, entry), "311");
  });

  test("a group without episode list is bad data", () => {
    badData(
      () => parseMylistByAnime(312, "Title|12|1|2|3|0|4|G1|1-6|G2"),
      "G2",
    );
  });

  test("a bad episode count is bad data", () => {
    badData(() => parseMylistByAnime(312, "Title|x|1|2|3|0|4"), "episodes");
  });
});

describe("numbers (9e)", () => {
  test("non numbers are bad data naming the field", () => {
    badData(
      () =>
        EPISODE_PARSER.parseData(
          "1|2|3|4|5|1|e|r|k|0|1".replace("1|2", "1|12x"),
        ),
      'bad number "12x" in field aid',
    );
    badData(() => MYLIST_PARSER.parseData("a|2|3|4|5|6|1|8|st|so|ot|0"), "lid");
    badData(() => ANIME_DESC_PARSER.parseData("x|1|text"), "current_part");
  });

  test("hex, exponents and decimals are rejected", () => {
    for (const bad of ["0x10", "1e3", "1.5", "NaN", "Infinity"]) {
      badData(() => ANIME_DESC_PARSER.parseData(`${bad}|1|text`));
    }
  });

  test("empty is 0, signs and spaces are fine", () => {
    const part = ANIME_DESC_PARSER.parseData("|+3|text");
    expect(part.current_part).toBe(0);
    expect(part.max_parts).toBe(3);
    expect(ANIME_DESC_PARSER.parseData(" 2 |1|t").current_part).toBe(2);
  });

  test("list items and nested records are checked", () => {
    badData(() => CHARACTER_PARSER.parseData("1|k|t|p|1,2,x,0|1,2|1|1|M"), "x");
    badData(() => CHARACTER_PARSER.parseData("1|k|t|p||1,y|1|1|M"), "y");
  });
});

describe("RANDOMANIME (18)", () => {
  const reply =
    "10914|26|26|2|850|120|845|30|870|4|1998|TV|R|K|E|o1'o2|s1|syn|1,2";

  test("the 19 spec fields parse", () => {
    const random = RANDOM_PARSER.parseData(reply);
    expect(random.aid).toBe("10914");
    expect(random.type).toBe("TV");
    expect(random.other_name_list).toEqual(["o1", "o2"]);
    expect(random.categories).toEqual(["1", "2"]);
    expect(Object.keys(random)).toHaveLength(19);
  });

  test("extra trailing fields are ignored", () => {
    expect(RANDOM_PARSER.parseData(`${reply}|900000000|0|5|1|pic.jpg`)).toEqual(
      RANDOM_PARSER.parseData(reply),
    );
  });

  test("too few fields are bad data", () => {
    badData(() => RANDOM_PARSER.parseData("10914|26"), "fields");
    badData(() =>
      RANDOM_PARSER.parseData(reply.slice(0, reply.lastIndexOf("|"))),
    );
  });
});

describe("bare 311 (19)", () => {
  test("no data is one edited entry", () => {
    expect(parseMylistEdited("")).toBe(1);
  });

  test("a count is kept", () => {
    expect(parseMylistEdited("3")).toBe(3);
    badData(() => parseMylistEdited("x"));
  });
});

describe("line breaks (21)", () => {
  test("episode, character, creator, group and mylist text", () => {
    expect(
      EPISODE_PARSER.parseData("1|2|3|4|5|1|E<br />ng|R<br />om|K<br />an|0|1"),
    ).toMatchObject({
      eng: "E\nng",
      romaji: "R\nom",
      kanji: "K\nan",
    });
    expect(
      CHARACTER_PARSER.parseData("1|k<br />1|t<br />1|p<br />1||1|1|1|M"),
    ).toMatchObject({
      character_name_kanji: "k\n1",
      character_name_transc: "t\n1",
    });
    expect(
      CREATOR_PARSER.parseData("1|k<br />1|t<br />1|1|p|u|u|w|w|1")
        .creator_name_kanji,
    ).toBe("k\n1");
    expect(
      GROUP_PARSER.parseData(GROUP.replace("Group Name", "A<br />B")).name,
    ).toBe("A\nB");
    expect(
      MYLIST_PARSER.parseData("1|2|3|4|5|6|1|8|s<br />t|s<br />o|o<br />t|0"),
    ).toMatchObject({ storage: "s\nt", source: "s\no", other: "o\nt" });
    expect(GROUP_STATUS_PARSER("1|G<br />x|1|2|3|4|1-2")[0]!.group_name).toBe(
      "G\nx",
    );
    expect(
      parseMylistByAnime(312, "T<br />x|12|1|2|3|0|4|G<br />1|1-6").multi,
    ).toMatchObject({ anime_title: "T\nx", groups: [{ group: "G\n1" }] });
    expect(
      RANDOM_PARSER.parseData(
        "1|2|2|0|0|0|0|0|0|0|1998|TV|a<br />b|K|E|o<br />1'o2|s|syn|1",
      ),
    ).toMatchObject({ romanji_name: "a\nb", other_name_list: ["o\n1", "o2"] });
  });
});
