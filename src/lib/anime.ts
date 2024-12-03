/*
 * Internal helper types and function for anime
 */

export type AnimeId = {
  anidb: number;
  anilist?: number;
  tvdb?: number;
  tvdbSeason?: number;
  tmdb?: number;
  tmdbSeason?: number;
};

export type AnimeTitleVariant = {
  title: string;
  type?: string;
  year?: number;
  language: string;
};

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
