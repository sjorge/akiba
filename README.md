# akiba

Akiba is an utility to export stub-NFO's for use with jellyfin.

Jellyfin does not like `adbren` resulting filenames, this tool can parse these and will query anidb to generate small stub NFO files.

When the anidb provider is enabled in jellyfin it will use the ID from the NFO to fetch the correct information. Additionally anilist IDs are attempted to be mapped. This is not always successful. But the anilist provider can also be used in most cases.

## install

To install dependencies:

```bash
bun install
```

## Usage

To run:

```bash
bun start
```
