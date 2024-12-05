declare module "anidbjs";

declare type AniDB_Show = {
  id: number;
  ageRestricted: boolean;
  type: string;
  episodeCount: number;
  startDate: string;
  endDate: string;
  titles: {
    title: string;
    type?: string;
    language: string;
  }[];
  description: string;
  picture: string;
  url: string;
  creators: {
    id: number;
    type: string;
    name: string;
  }[];
  tags: {
    id: number;
    weight: number;
    localSpoiler: boolean;
    globalSpoiler: boolean;
    name: string;
    description: string;
    updatedAt: string;
    pictureUrl?: string;
  }[];
  characters: {
    id: number;
    type: string;
    updatedAt: string;
    rating: number;
    votes: number;
    name: string;
    gender: string;
    characterType: {
      id: number;
      name: string;
    };
    description: string;
    picture?: string;
    seiyuu: {
      id: number;
      picture?: string;
      name: string;
    }[];
  }[];
  episodes: {
    id: number;
    updatedAt: string;
    episodeNumber: string;
    type: number;
    length: number;
    airDate: string;
    rating?: number;
    votes?: number;
    titles: {
      title: string;
      type?: string;
      language: string;
    }[];
    summary?: string;
  }[];
};

declare class AniDB {
  constructor(
    credentials: {
      client: string;
      version: string;
    },
    options?: {
      baseUrl?: string;
      timeout?: string;
      agent?: string;
      headers?: {
        [name: string]: string;
      };
    },
  );
  opts?: unknown;
  queryParams?: {
    client: string;
    clientver: string;
    protover: number;
  };

  set options(opts: unknown);

  anime(id: number): Promise<AniDB_Show>;

  randomRecommendation(): Promise<AniDB_Show>;
}

declare class httpError {
  constructor(message: string, code: number);
  name: string;
  code: string;
  status: string;
  toString(): string;
  toJSON(): {
    code: string;
    status: string;
    message: string;
  };
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
