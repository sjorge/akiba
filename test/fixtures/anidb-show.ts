/*
 * Hand built AniDB show used by the nfo tests
 */

export function makeShow(overrides: Partial<AniDB_Show> = {}): AniDB_Show {
  return {
    id: 100,
    ageRestricted: false,
    type: "TV Series",
    episodeCount: 3,
    startDate: "2020-01",
    endDate: "2020-03-25",
    titles: [
      { title: "Test `Anime` EN", type: "official", language: "en" },
      { title: "Test `Anime`", type: "main", language: "x-jat" },
      { title: "テスト", type: "official", language: "ja" },
    ],
    description: "A plot",
    picture: "12345.jpg",
    url: "https://anidb.net/anime/100",
    creators: [],
    tags: [],
    characters: [],
    episodes: [
      {
        id: 1001,
        updatedAt: "2020-01-01",
        episodeNumber: "1",
        type: 1,
        length: 24,
        airDate: "2020-01-08",
        titles: [
          { title: "First `Ep`", language: "en" },
          { title: "第一話", language: "ja" },
        ],
      },
      {
        id: 1002,
        updatedAt: "2020-01-01",
        episodeNumber: "2",
        type: 1,
        length: 24,
        airDate: "2020-01",
        titles: [{ title: "Second", language: "en" }],
      },
      {
        id: 1003,
        updatedAt: "2020-01-01",
        episodeNumber: "3",
        type: 1,
        length: 24,
        airDate: "",
        titles: [{ title: "Third", language: "en" }],
      },
      {
        id: 2001,
        updatedAt: "2020-01-01",
        episodeNumber: "S1",
        type: 2,
        length: 24,
        airDate: "2020-04-01",
        titles: [{ title: "Special", language: "en" }],
      },
    ],
    ...overrides,
  };
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
