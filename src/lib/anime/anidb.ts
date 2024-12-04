/*
 * Internal helper types and function for anidb
 */
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import toml from "@iarna/toml";
import axios from "axios";
import { convert as convertXmlDoc } from "xmlbuilder2";
import { deepmergeInto } from "deepmerge-ts";

import type { Config } from "lib/config";
import type { AnimeId, AnimeTitleVariant } from "lib/anime";

const TitleURL = "https://anidb.net/api/anime-titles.xml.gz";

type TitleXMLAnimeTitle = {
        "@type": string;
        "@xml:lang": string;
        "#": string;
};

type TitleXML = {
  animetitles: {
    anime: {
      "@aid": number;
      title: TitleXMLAnimeTitle | TitleXMLAnimeTitle[];
    }[];
  };
};

type TitleMapping = {
  [anidb: number]: AnimeTitleVariant[];
};

export class AniDBResolver {
  private titleFile: string;
  private titleCacheAge: number;
  private titleCache: TitleMapping = {};

  public constructor(config: Config) {
    this.titleCacheAge = config.cache.anidb_age;
    this.titleFile = path.join(config.cache.path, "titles.map.toml");
  }

  public toString(): string {
    return "AniDBResolver";
  }

  public async refresh(): Promise<void> {
    // load from cache if fresh
    if (fs.existsSync(this.titleFile)) {
      const cacheStats = fs.statSync(this.titleFile);
      if (
        (new Date().getTime() - cacheStats.mtimeMs) / 1000 / 3600 / 24 <
        this.titleCacheAge
      ) {
        deepmergeInto(
          this.titleCache,
          toml.parse(fs.readFileSync(this.titleFile, "utf8")) as TitleMapping,
        );
        return;
      }
    }

    // load from remote
    this.titleCache = {};

    const titles = await axios.get(TitleURL, { responseType: "arraybuffer" });
    const titlesXml = convertXmlDoc(zlib.gunzipSync(titles.data).toString(), {
      format: "object",
    }) as unknown as TitleXML;

    for (const entry of titlesXml.animetitles.anime) {
      const aid: number = entry["@aid"];

      if (this.titleCache[aid] === undefined) this.titleCache[aid] = [];

      // ensure single TitleXMLAnimeTitle is mapped as TitleXMLAnimeTitle[]
      if ((entry.title as TitleXMLAnimeTitle)["#"]) {
        entry.title = [entry.title] as TitleXMLAnimeTitle[];
      }

      for (const entryTitle of entry.title as TitleXMLAnimeTitle[]) {
        const title: AnimeTitleVariant = {
          title: entryTitle["#"],
          type: entryTitle["@type"],
          language: entryTitle["@xml:lang"],
        };
        if (
          title.type !== undefined &&
          ["official", "main"].includes(title.type)
        ) {
          this.titleCache[aid].push(title);
        }
      }
    }

    // ensure cache dir exists
    fs.mkdirSync(path.dirname(this.titleFile), {
      recursive: true,
      mode: 0o750,
    });

    // write cache
    fs.writeFileSync(this.titleFile, toml.stringify(this.titleCache), {
      encoding: "utf8",
    });
    fs.chmodSync(this.titleFile, 0o660);
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
