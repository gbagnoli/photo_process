# photo_process

A tool to process photo collections: geotagging, time synchronization, renaming, and organization.

## Prerequisites

You need the following tools working and reachable in your `$PATH`:

*   [ExifTool](https://exiftool.org/)
*   [gpicsync](https://github.com/h4tr3d/gpicsync)
*   [garmin-cli](https://github.com/vicentereig/garmin-cli): Install via `cargo install garmin-cli`. You must authenticate once with `garmin auth login` before using GPX download features.

## Build

```bash
cargo build --release
```

## Run

The tool provides several commands. The `process` command is the most comprehensive:

```bash
cargo run --release -- process -z <timezone> /path/to/pics
```

See `GEMINI.md` or run with `--help` for more details on available commands and options.
