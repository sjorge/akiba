# akiba style guide

How code in this repository is written. It describes what `src/` does today;
follow the dominant pattern in new code and list differences under
"Known inconsistencies" instead of inventing rules. It is meant for people and
for LLM assistants alike: the checklist comes first, the details follow.

## Checklist

- Format with Prettier defaults (2 spaces, double quotes, semicolons, trailing
  commas, 80 columns). Lint with `bun run lint`, type check with
  `bun run check`, both must be clean, and `bun test` must pass.
- Import application modules by bare path (`lib/config`), never with `../`.
  Node built-ins use the `node:` prefix. Type-only imports use `import type`.
- Every function has an explicit return type. No `any`.
- Types are `type` aliases, not `interface`.
- Library code throws, command code catches, logs and sets
  `process.exitCode = 1` (never `process.exit()`).
- User facing output goes through `log()` from `lib/logger`, not `console.log`.
- Function docs are `/* * ... */` blocks (not JSDoc), inline comments are
  lower-case `//`. Markers are `NOTE:` and `XXX:`, not `TODO:`.
- Classes mark every member `public` or `private`.
- Every `.ts` file ends with the Vim modeline.
- `src/lib/anidb/` imports nothing from outside its own directory.

## 1. Tooling and formatting

- Runtime and package manager: Bun. ES modules, TypeScript `strict`, target
  `ESNext`, `moduleResolution: bundler`, `verbatimModuleSyntax: true`, so
  type-only imports must be `import type`.
- Prettier runs with no config file (`bun run pretty:check` / `pretty:write`,
  scope `src/` and `test/`). It is not in `devDependencies`, it runs through
  `bun x`.
- ESLint (`eslint.config.mjs`): `eslint:recommended` and `typescript-eslint`
  recommended, `--max-warnings=0`, plus `explicit-function-return-type`,
  `no-explicit-any`, `no-unused-vars`, `no-floating-promises`,
  `await-thenable`, `ban-ts-comment` and `return-await: always`. Prettier's
  config is applied last and switches the formatting rules off.
- `noUnusedLocals` and `noUnusedParameters` are off in `tsconfig.json`, ESLint
  catches unused variables instead.
- Every `.ts` file under `src/` ends with:

  ```ts
  // vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
  ```

## 2. Layout

```
src/cli.ts              Commander program, registers add*Command
src/vars.ts             program name and version
src/cmd/<name>.ts       one file per subcommand
src/lib/*.ts            shared helpers (config, logger, nfo, ed2khash, anime types)
src/lib/anime/*.ts      anime domain classes (resolver, renamer, metadata, nfo, formatter)
src/lib/anime/idmap/    ID mapping sources (local, list, anilist, tmdb)
src/lib/anidb/          AniDB UDP API client (self contained, see section 11)
test/                   bun tests, mirrors src/ (see section 12)
types/                  ambient .d.ts for untyped packages (anidbjs.d.ts)
```

- File names are lower-case and hyphenated (`write-nfo.ts`, `rate-limiter.ts`),
  one primary class or topic per file.
- `lib/anime.ts` (a file) sits next to the directory `lib/anime/` and holds the
  shared types and constants.
- Command files export `<name>Action` (the handler) first and
  `add<Name>Command` (registration) second. `cmd/configure.ts` and
  `cmd/idmap.ts` keep the handler non-exported.
- File order: header comment, imports, local types and constants, helpers,
  exported types, exported class or functions, modeline.

## 3. Imports

- Application modules use bare paths resolved from `baseUrl: ./src`
  (`lib/config`, `lib/anime/resolver`, `cmd/rename`, `vars`).
- Node built-ins use `node:` (`node:fs`, `node:path`, `node:zlib`).
- Default imports where a module has a default (`fs`, `path`, `toml`,
  `axios`), named imports otherwise.
- Order, with a blank line between groups:
  1. `vars` (when needed)
  2. node built-ins and third-party packages
  3. `import type { ... } from "lib/..."`
  4. value imports from `lib/...` and `cmd/...`

  ```ts
  import fs from "node:fs";
  import toml from "@iarna/toml";

  import type { Config } from "lib/config";
  import type { AnimeId } from "lib/anime";

  import { AnimeResolver } from "lib/anime/resolver";
  ```

  `import type { X }` is its own statement. The one inline `type` modifier is
  `import { animeStringFormat, type AnimeStringFormatData }` in
  `lib/anime/renamer.ts`.

## 4. Naming

| Thing | Style | Example |
| --- | --- | --- |
| Files | `kebab-case.ts` | `write-nfo.ts`, `ed2khash.ts` |
| Classes | `PascalCase`, `Anime` prefix for domain classes | `AnimeRenamer`, `AnimeIdmapLocal` |
| Exception classes | `<Class>Exception extends Error` | `AnimeIdmapTmdbException` |
| Types | `PascalCase` `type` aliases | `AnimeId`, `Ed2kHash` |
| Functions | `camelCase`, verb first | `readConfig`, `animeStringFormat` |
| Command handlers | `<name>Action`, `add<Name>Command` | `renameAction` |
| Module constants | `UPPER_SNAKE` for lists and flags, `PascalCase`/`camelCase` for others | `IGNORE_EXTENSIONS`, `TitlesDBUrl` |
| Private members | `camelCase`, `private` keyword, no `_` prefix | `this.hashCache` |
| Build-time vars | `_DEFINE_` prefix | `_DEFINE_PROG` |
| Caught error | `_e`, then `const e = _e as T` | section 7 |
| Data from external systems | keep the upstream or `snake_case` name | `anime_name_romaji`, `http_client` |

Types that map to template placeholders or TOML keys (`AnimeStringFormatData`,
the config types) and the result types of the AniDB client use `snake_case`
keys, so `camelCase` does not apply inside them.

## 5. Classes versus functions

- Stateful domain objects are **classes** with a `public constructor(config:
  Config, ...)`, an explicit `public` or `private` on every member, typed
  properties at the top, and `Config` passed in (classes do not read config
  themselves):

  ```ts
  export class AnimeIdmapLocal {
    private mappingFile: string;
    private mappingCache: LocalMapping = {};

    public constructor(config: Config) { ... }
    public toString(): string { return "AnimeIdmapLocal"; }
    public async refresh(): Promise<boolean> { ... }
    public apply(id: AnimeId, overwrite: boolean = false): void { ... }
  }
  ```

- Classes define `public toString(): string` returning the class name
  (renamer, metadata, resolver, nfo, idmap sources, the AniDB client and its
  helpers). Exception classes do not.
- Initialisation that needs I/O is a separate `async refresh()` or `connect()`,
  never done in the constructor.
- Stateless helpers and command handlers are plain functions. A helper used
  once is nested in the function that uses it (`sanitizeTag` in
  `animeStringFormat`).
- Inheritance only for the NFO classes and for `Error` subclasses (and the
  masked parsers of the AniDB client).

## 6. Types and signatures

- Explicit return type on every function, including `void` and
  `Promise<void>`.
- Exported constants carry a type annotation
  (`export const _DEFINE_PROG: string = "akiba";`). `as const` lookup tables
  are the exception, `as const` is their type.
- Optional parameters use a default with an explicit type:
  `overwrite: boolean = false`.
- No `any`. Declarations for untyped packages go in `types/`.
- Assertions (`as T`) are normal for TOML results, commander options and
  caught errors.
- Option and result shapes are `type` aliases with optional (`?`) fields, not
  classes.

## 7. Error handling

- **Library code throws**: a plain `Error` or a dedicated exception class.

  ```ts
  export class AnimeIdmapTmdbException extends Error {
    constructor(message: string) {
      super(message);
      this.name = "AnimeIdmapTmdbException";
    }
  }
  ```

- **Command code catches, logs and sets the exit code**, it does not call
  `process.exit()`:

  ```ts
  } catch (_e: unknown) {
    const e = _e as Error;
    log(`${msg}: ${e.message}`, "error");
    process.exitCode = 1;
  }
  ```

- Catch clauses are typed `unknown` and cast afterwards, the parameter is
  `_e`. Rethrow with `throw _e;` or `throw _e as Error;`.
- Best effort operations use a bare `catch {` and return a fallback
  (`false` or `undefined`).
- Validation failures log, return a boolean, and the caller sets
  `process.exitCode = 1`.
- Commander argument parsers throw `InvalidArgumentError`.
- Expected "not found" answers from remote systems are returned as
  `undefined`, not thrown (`getAniDBFileByHash` for code 320).

## 8. Logging and output

- All user facing output goes through `log(msg, type, showAnimeIds?, anime?)`
  from `lib/logger`. `type` is `"error" | "warn" | "step" | "done" | "info"`
  (default `"info"`); it handles TTY colours and the `[!!]`, `[WW]`, `[II]`,
  `[OK]`, `[>>]` prefixes. `banner()` prints once per process.
- A long action logs `log("X ...", "step")` and then
  `log("X ...", "done" | "error")` with the same text.
- `console.log` is only for deliberate raw output (`--print-ed2klinks`,
  `configure`'s JSON dump), each with a comment saying so.
- Library code that is meant to be reusable does not print at all, it reports
  through a callback (`onWarning` in the AniDB client) and the caller decides.

## 9. Comments

- File header: a block comment.

  ```ts
  /*
   * Internal helper types and function for anidb
   */
  ```

  Most `lib/` files have one.
- Function, method, class and exported constant docs are `/* * ... */`
  blocks, not JSDoc, wrapped at 80 columns.
- Inline comments are lower-case `//` and explain a step
  (`// sanity check parameters`, `// ensure cache dir exists`).
- Markers: `NOTE:` for explanations and constraints, `XXX:` for a known
  problem or open question. No `TODO:` or `FIXME:`.
- Where code follows an external spec, say so in the doc comment: the request
  form, the reply codes, the constraints. Mark deliberate differences from the
  spec with `NOTE:` and the reason.
- `eslint-disable` is rare and always names the rule.

## 10. Async and I/O

- `async`/`await` throughout; callbacks and event emitters are not used in
  akiba's own code (`.then()` appears once, in `lib/nfo.ts`).
- File system work is synchronous (`readFileSync`, `existsSync`, `mkdirSync`)
  except hashing (`generateEd2kHash`, streamed) and network I/O.
- HTTP uses `axios`, AniDB HTTP uses `anidbjs`, TOML uses `@iarna/toml`,
  defaults are merged with `deepmerge-ts`.
- Network backed classes cache under the configured cache path and compare file
  mtime with a configured age in days (`cacheAge*`).
- Directories are created with mode `0o750`, files with `0o660` (the config
  file `0o600`).

## 11. `src/lib/anidb/`

The AniDB UDP API client is written to be split out into its own library
later, so it follows a few extra rules:

- It imports only from `lib/anidb/...` and Node. It never imports the logger,
  config or anything else from the rest of `src/`. Problems that do not stop it
  are reported through the `onWarning` option, fatal ones are thrown as
  `AnidbError`.
- Names that mirror the AniDB API (`incomming_port`, `romanji_name`, result
  keys, command method names such as `file_by_hash`) keep their upstream
  spelling and `snake_case`, even where they are misspelled. Do not rename
  them casually, they are the public API.
- The module layout is `client.ts` (the commands), `responses.ts` (per command
  record layouts), `parsers/` (masks and field types), `udp/` (socket, rate
  limiter, protocol, compression, encryption), `constants.ts`, `types.ts`,
  `error.ts`.
- Doc comments name the command, the reply codes and the constraints of the
  AniDB "UDP API Definition" (the wiki page). Keep the request spacing
  (`REQUEST_INTERVAL_MS`) and the single request in flight, they implement the
  spec's flood protection.

## 12. Tests

- Tests run with `bun test` (`bunfig.toml` sets `root = "test"` and a preload
  that points the config and cache at a temp directory).
- `test/` mirrors `src/`: `src/lib/anime/formatter.ts` is tested by
  `test/lib/anime/formatter.test.ts`. Files are named `<name>.test.ts`.
- Tests follow the same rules as `src/`: bare imports (`lib/...`,
  `test/helpers`), explicit return types on every callback, and the modeline.
- No network. Seed caches with `seedCacheFile` and build config with
  `makeConfig` from `test/helpers.ts`, and use `makeTempDir` for any file
  system work.
- `src/lib/anidb/` has no tests here, do not add any.

## Known inconsistencies

These are observed, not rules:

- `eslint.config.mjs` and `package.json` indent with 4 spaces, `src/` with 2.
  `eslint.config.mjs` sets `object-curly-spacing: never`, which
  `eslint-config-prettier` switches off.
- Header comments are missing from `cmd/*`, `cli.ts`, `vars.ts`, `logger.ts`
  and `config.ts`.
- Misspellings that are part of public names or output and must not be renamed
  casually: `Threshhold`, `quaility`, `orginal_name`, `mintues`,
  `desination`.
- The target type of `_e as X` differs per call site (`Error`, `AnidbError`,
  `httpError`).
- `logger.ts` writes `error` to stderr on a TTY but to stdout otherwise.
- Exception class constructors omit `public`, other classes do not.
- `export const` naming is mixed (`configFile`, `ANIME_EPISODE_EXTENSIONS`,
  `_DEFINE_*`, `TitlesDBUrl`).
- Prettier is a de facto requirement but not a declared dependency.
