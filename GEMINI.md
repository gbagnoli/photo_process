# photo_process

## Project Overview

`photo_process` is a tool for managing and processing photo collections. It primarily focuses on geotagging, time synchronization (timezone adjustments), renaming, and organizing images. It is implemented in Rust as a comprehensive CLI application.

The tool relies heavily on external CLI utilities to perform file manipulations and metadata updates.

## Prerequisites

The following tools must be installed and available in your system's `$PATH`:

*   **[ExifTool](https://exiftool.org/)**: Used for reading and writing metadata (EXIF tags).
*   **[gpicsync](https://github.com/h4tr3d/gpicsync)**: Used for geotagging images based on GPX files.

## Building and Running

The source code is located in `src/main.rs`.

**Build:**
```bash
cargo build --release
```

**Run:**
```bash
# Run via cargo
cargo run --release -- <command> [options]

# Example: Run the 'all' command
cargo run --release -- all --gps-files path/to/track.gpx path/to/images/*.jpg
```

**Key Commands:**
*   `process`: Comprehensive workflow (Shift to UTC -> Organize -> Geotag -> Set Time -> Rename).
*   `organize`: Organize photos into directories by date.
*   `geotag`: Geotag images using GPX files.
*   `set-time`: Set time and timezone on pictures.
*   `shift-to-utc`: Detect timezone from photos and shift to UTC.
*   `rename`: Rename images based on date/time.

## Development Conventions

*   **Rust Workflow**: After any changes to the code, you **must** run the following commands to ensure code quality and style adherence:
    ```bash
    cargo fmt
    cargo check
    cargo clippy
    ```
*   **External Tool Wrappers**: Much of the logic involves constructing and executing arguments for `exiftool` and `gpicsync`.
*   **Timezones**: Timezones are handled via a predefined list of cities (e.g., "Dublin", "New York") mapping to offsets.
*   **File Handling**: Operations often involve recursive directory scanning and pattern matching for image extensions (default: jpg, mp4).