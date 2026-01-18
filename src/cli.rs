use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "photo_process")]
#[command(about = "simple scripts to process photos")]
pub struct Cli {
    #[arg(long, default_value_t = 10)]
    pub timerange: u64,

    #[arg(
        short = 'e',
        long,
        default_value = "jpg,JPG,mp4",
        value_delimiter = ','
    )]
    pub suffix: Vec<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Rename images using their date and time
    Rename {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// set time and timezone on pictures
    SetTime {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        #[arg(short = 'z', long, required = true)]
        timezone: String,
        #[arg(long, default_value_t = false)]
        dst: bool,
    },
    /// geotag images using gpx files
    Geotag {
        #[arg(short = 'g', long, required = true)]
        gps_files: Vec<PathBuf>,
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// shift photos - this will also clear out timezones
    Shift {
        #[arg(long, default_value_t = false)]
        reset_tz: bool,
        by: String,
        paths: Vec<PathBuf>,
    },
    /// detect timezone from photos and shift to UTC
    ShiftToUtc {
        /// Files or directories to process
        paths: Vec<PathBuf>,
    },
    /// detect timezone from photos in directories
    DetectTimezone {
        /// Files or directories to process
        paths: Vec<PathBuf>,
    },
    /// Organize photos into directories by date (YYYY-MM-DD)
    Organize {
        /// Directories to organize
        #[arg(required = true)]
        dirs: Vec<PathBuf>,
    },
    /// Process photos: Shift to UTC, Organize, Geotag, Set Time (with DST), Rename
    Process {
        #[arg(long, default_value_t = false)]
        force: bool,
        /// Directories to process
        #[arg(required = true)]
        dirs: Vec<PathBuf>,
        #[arg(short = 'z', long, required = true)]
        timezone: String,
        #[arg(long, default_value_t = false)]
        dst: bool,
        /// Run organization step
        #[arg(long, default_value_t = false)]
        organize: bool,
    },
    /// Download GPX files from Garmin
    DownloadGpx {
        /// Destination directory
        #[arg(required = true)]
        dest: PathBuf,
        /// Start date (YYYY-MM-DD), defaults to 20 days ago
        #[arg(long)]
        start_date: Option<String>,
        /// End date (YYYY-MM-DD), defaults to today
        #[arg(long)]
        end_date: Option<String>,
    },
}
