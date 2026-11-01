import { expect, test } from "bun:test";

import { AnidbError } from "lib/anidb/error";
import { AnimeParser, FileParser } from "lib/anidb/parsers";

test("ANIME mask: text fields decode line breaks, numbers are checked", () => {
  const parser = new AnimeParser([
    "romanji_name",
    "other_name_list",
    "episodes",
  ]);
  // aid, romanji, other, episodes (mask order)
  expect(parser.fields).toEqual([
    "aid",
    "romanji_name",
    "other_name_list",
    "episodes",
  ]);
  const parsed = parser.parseData("7|a<br />b|o<br />1'o2|12");
  expect(parsed).toMatchObject({
    aid: 7,
    romanji_name: "a\nb",
    other_name_list: ["o\n1", "o2"],
    episodes: 12,
  });
  expect(() => parser.parseData("7|a|o|12x")).toThrow(AnidbError);
  expect(() => parser.parseData("seven|a|o|12")).toThrow(
    /bad number "seven" in field aid/,
  );
});

test("FILE mask: description, names and episode text decode line breaks", () => {
  const parser = new FileParser([
    "description",
    "english_name",
    "ep_name",
    "group_name",
    "size",
  ]);
  const data = parser.fields
    .map(
      (f) =>
        ({
          size: "1234",
          description: "d<br />1",
          english_name: "e<br />n",
          ep_name: "p<br />n",
          group_name: "g<br />n",
        })[f as string]!,
    )
    .join("|");
  expect(parser.parseData(`99|${data}`)).toMatchObject({
    fid: 99,
    description: "d\n1",
    english_name: "e\nn",
    ep_name: "p\nn",
    group_name: "g\nn",
    size: 1234,
  });
});

test("FILE mask: a bad file id or number is bad data", () => {
  const parser = new FileParser(["size"]);
  expect(() => parser.parseData("x|1")).toThrow(AnidbError);
  expect(() => parser.parseData("1|abc")).toThrow(
    /bad number "abc" in field size/,
  );
});
