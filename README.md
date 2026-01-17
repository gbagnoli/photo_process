# photo_process

A tool to process photo collections: geotagging, time synchronization, renaming, and organization.

## Prerequisites

You need the following tools working and reachable in your `$PATH`:

*   [ExifTool](https://exiftool.org/)
*   [GPSBabel](https://www.gpsbabel.org/)
*   [gpicsync](https://github.com/h4tr3d/gpicsync)

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