/*
 * Internal helper types and function for anidb
 */
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import toml from "@iarna/toml";
import axios from "axios";
import he from "he";
import { convert as convertXmlDoc } from "xmlbuilder2";
import { deepmergeInto } from "deepmerge-ts";
import levenshtein from "fast-levenshtein";
import naturalCompare from "natural-compare";

import type { Config } from "lib/config";
import type { AnimeId, AnimeTitleVariant } from "lib/anime";

import { ANIME_EPISODE_EXTENSIONS } from "lib/anime";

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

const titleAidRegEx = new RegExp(/\[anidb-(?<aid>\d+)\]/);

// WARN: we episodeStart/episodeEnd need to be strings
//       to support things like S1, OP1, ...
export type EpisodeFile = {
  path: string;
  episodeStart: string;
  episodeEnd: string;
  title: string;
};

const episodeSingleTitleRegEx = new RegExp(
  /\s-\s(?<episode>(S|C|T|P|O|E)?\d+)\s-\s(?<title>.+)\.\w{3}/,
);
const episodeMultiTitleRegEx = new RegExp(
  /\s-\s(?<episode>(S|C|T|P|O)?\d+-(S|C|T|P|O)?\d+)\s-\s(?<title>.+)\.\w{3}/,
);
const episodeCrc32RegEx = new RegExp(/.+(?<crc32>\([A-Za-z0-9]{8}\))/);

export class AnimeResolver {
  private titleFile: string;
  private titleCacheAge: number;
  private titleCache: TitleMapping = {};
  private fuzzyMatchThreshhold: number = 3;

  public constructor(config: Config) {
    this.titleCacheAge = config.cache.title_age;
    this.titleFile = path.join(config.cache.path, "titles.map.toml");
  }

  public toString(): string {
    return "AnimeResolver";
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
    const titlesXml = JSON.parse(
      convertXmlDoc(zlib.gunzipSync(titles.data).toString(), {
        format: "json",
      }),
    ) as TitlesDBObject;

    for (const entry of titlesXml.animetitles.anime) {
      const aid: number = entry["@aid"];

      if (this.titleCache[aid] === undefined) this.titleCache[aid] = [];

      // ensure single TitlesDBTitleObject is mapped as TitlesDBTitleObject[]
      if ((entry.title as TitlesDBTitleObject)["#"]) {
        entry.title = [entry.title] as TitlesDBTitleObject[];
      }

      for (const entryTitle of entry.title as TitlesDBTitleObject[]) {
        const title: AnimeTitleVariant = {
          title: he.decode(entryTitle["#"]),
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

  public title(id: AnimeId): AnimeTitleVariant[] {
    return this.titleCache[id.anidb];
  }

  public id(title: string): AnimeId | undefined {
    // match [anidb-<aid>] tag
    const aidMatch: RegExpExecArray | null = titleAidRegEx.exec(title);
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

  public episodes(animePath: string): EpisodeFile[] {
    const episodes: EpisodeFile[] = [];

    // check path exists
    if (!fs.existsSync(animePath)) return [];
    if (!fs.statSync(animePath).isDirectory()) return [];

    // read anime show path
    for (const episodePath of fs.readdirSync(animePath).sort(naturalCompare)) {
      if (!ANIME_EPISODE_EXTENSIONS.includes(path.extname(episodePath)))
        continue;

      const singleEpisode = episodeSingleTitleRegEx.exec(episodePath)?.groups;
      const multiEpisode = episodeMultiTitleRegEx.exec(episodePath)?.groups;

      let title: string | undefined = undefined;
      let episode: string[] = [];
      if (singleEpisode) {
        title = singleEpisode?.title;
        episode = [
          singleEpisode?.episode.startsWith("0")
            ? `${parseInt(singleEpisode?.episode)}`
            : `${singleEpisode?.episode}`,
        ];
      } else if (multiEpisode) {
        title = multiEpisode?.title;
        episode = multiEpisode?.episode.split("-").map((ep: string): string => {
          return `${parseInt(ep)}`;
        });
      }

      // skip if we couldn't parse the episode filename
      if (title === undefined) continue;

      // strip checkum from filename
      const checksum = episodeCrc32RegEx.exec(title)?.groups;
      if (checksum) title = title.replace(checksum?.crc32, "");

      // add episode
      episodes.push({
        path: path.join(animePath, episodePath),
        episodeStart: episode[0],
        episodeEnd: episode[episode.length - 1],
        title: title.trim(),
      } as EpisodeFile);
    }

    return episodes;
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
