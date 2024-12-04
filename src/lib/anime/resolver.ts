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
import levenshtein from "fast-levenshtein";

import type { Config } from "lib/config";
import type { AnimeId, AnimeTitleVariant } from "lib/anime";

const TitlesDBUrl = "https://anidb.net/api/anime-titles.xml.gz";

type TitlesDBTitleObject = {
  "@type": string;
  "@xml:lang": string;
  "#": string;
};

type TitlesDBObject = {
  animetitles: {
    anime: {
      "@aid": number;
      title: TitlesDBTitleObject | TitlesDBTitleObject[];
    }[];
  };
};

type TitleMapping = {
  [anidb: number]: AnimeTitleVariant[];
};

const TitleAidRegEx = new RegExp(/\[anidb-(?<aid>\d+)\]/);

export class AnimeResolver {
  private titleFile: string;
  private titleCacheAge: number;
  private titleCache: TitleMapping = {};
  private fuzzyMatchThreshhold: number = 3;

  public constructor(config: Config) {
    this.titleCacheAge = config.cache.mapping_age;
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

    const titles = await axios.get(TitlesDBUrl, {
      responseType: "arraybuffer",
    });
    const titlesXml = convertXmlDoc(zlib.gunzipSync(titles.data).toString(), {
      format: "object",
    }) as unknown as TitlesDBObject;

    for (const entry of titlesXml.animetitles.anime) {
      const aid: number = entry["@aid"];

      if (this.titleCache[aid] === undefined) this.titleCache[aid] = [];

      // ensure single TitlesDBTitleObject is mapped as TitlesDBTitleObject[]
      if ((entry.title as TitlesDBTitleObject)["#"]) {
        entry.title = [entry.title] as TitlesDBTitleObject[];
      }

      for (const entryTitle of entry.title as TitlesDBTitleObject[]) {
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

  public titleFromId(id: AnimeId): AnimeTitleVariant[] {
    return this.titleCache[id.anidb];
  }

  public resolveFromTitle(title: string): AnimeId | undefined {
    // match [anidb-<aid>] tag
    const aidMatch: RegExpExecArray | null = TitleAidRegEx.exec(title);
    if (aidMatch !== null) {
      return { anidb: parseInt(aidMatch[1]) } as AnimeId;
    }

    // fuzzy search
    const titleNormalized: string = title.replace("⁄", "/");

    let exact_match: number | undefined;
    let best_match: number | undefined;
    let best_match_score: number = 0;

    Object.entries(this.titleCache).forEach(([key, value]) => {
      const aid: number = parseInt(key);
      const titles: AnimeTitleVariant[] = value;
      titles.forEach((variant: AnimeTitleVariant) => {
        if (variant.title == titleNormalized) {
          exact_match = aid;
        } else {
          const distance: number = levenshtein.get(
            titleNormalized,
            variant.title,
            { useCollator: true },
          );
          if (distance <= this.fuzzyMatchThreshhold) {
            if (best_match == undefined || best_match_score > distance) {
              best_match = aid;
              best_match_score = distance;
            }
          }
        }
      });
    });

    if (exact_match !== undefined) {
      return { anidb: exact_match } as AnimeId;
    } else if (best_match !== undefined) {
      return { anidb: best_match } as AnimeId;
    }

    return undefined;
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
