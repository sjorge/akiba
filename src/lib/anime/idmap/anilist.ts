/*
 * Internal helpers for dealing with anilist api
 */
import type { Config } from "lib/config";
import type { AnimeId } from "lib/anime";

export class AnimeIdmapAnilist {
  public constructor(config: Config) {
    // XXX: connect to anilist ?
    console.log(config.anilist.token);
  }

  public toString(): string {
    return "AnimeIdmapAnilist";
  }

  public lookup(title: string, id: AnimeId, overwrite: boolean = false): void {
    if (id.anilist === undefined || overwrite) {
      console.log(`TODO: Search for ${title} ...`);
    }
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
