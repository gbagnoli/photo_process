use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// --- Constants & Config ---

const TZ_CITIES_DATA: &[(&str, i32, &str)] = &[
    ("Austin", 28, "-06:00"),
    ("Buenos Aires", 25, "-04:00"),
    ("Dublin", 20, "+00:00"),
    ("Galapagos", 28, "-06:00"),
    ("London", 20, "+00:00"),
    ("Mexico City", 28, "-06:00"),
    ("New York", 27, "-05:00"),
    ("Rome", 19, "+01:00"),
    ("Quintana Roo", 27, "-05:00"),
    ("Quito", 27, "-05:00"),
    ("San Francisco", 30, "-08:00"),
    ("Santiago", 25, "-04:00"),
    ("Singapore", 7, "+08:00"),
    ("Kiev", 17, "+02:00"),
    ("US/Central", 28, "-06:00"),
    ("US/Eastern", 27, "-05:00"),
    ("US/Pacific", 30, "-08:00"),
];

#[derive(Debug, Clone)]
struct AppConfig {
    suffixes: Vec<String>,
    images_dir: PathBuf,
    timerange: u64,
    timezone: String,
    timezone_dst: bool,
    timezone_id: i32,
}

// --- CLI Definitions ---

#[derive(Parser)]
#[command(name = "photo_process")]
#[command(about = "simple scripts to process photos")]
struct Cli {
    #[arg(short = 'd', long, default_value = ".")]
    images_dir: PathBuf,

    #[arg(short = 'z', long, default_value = "Dublin")]
    timezone: String,

    #[arg(long, default_value_t = false)]
    dst: bool,

    #[arg(long, default_value_t = 10)]
    timerange: u64,

    #[arg(short = 'e', long, default_value = "jpg,mp4", value_delimiter = ',')]
    suffix: Vec<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Rename images using their date and time
    Rename,
    /// set time and timezone on pictures
    SetTime,
    /// geotag images using gpx files
    Geotag {
        #[arg(required = true)]
        gps_files: Vec<PathBuf>,
    },
    /// shift photos - this will also clear out timezones
    Shift {
        #[arg(long, default_value_t = false)]
        reset_tz: bool,
        by: String,
        images: Vec<PathBuf>,
    },
    /// set GPS coordinates on images
    SetGps {
        #[arg(long, default_value = "N")]
        latitude_ref: String,
        #[arg(long, default_value = "E")]
        longitude_ref: String,
        lat: String,
        log: String,
        images: Vec<PathBuf>,
    },
    /// Run all: geotag, set_time, rename
    All {
        gps_files: Vec<PathBuf>,
    },
}

// --- Helpers ---

fn run(program: &str, args: &[&str]) -> Result<()> {
    println!("Running: {} {}", program, args.join(" "));
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| "Failed to execute command")?;

    if !status.success() {
        return Err(anyhow::anyhow!("Command exited with non-zero status"));
    }
    Ok(())
}

fn get_tz_info(city: &str) -> Result<(i32, String)> {
    for (name, id, offset) in TZ_CITIES_DATA {
        if *name == city {
            return Ok((*id, offset.to_string()));
        }
    }
    Err(anyhow::anyhow!("Unknown timezone city: {}", city))
}

fn gpx_name(gps_file: &Path) -> Result<PathBuf> {
    if gps_file.extension().and_then(|e| e.to_str()) != Some("gpx") {
        let mut dest = gps_file.parent().unwrap_or(Path::new(".")).to_path_buf();
        dest.push(format!("{}.gpx", gps_file.file_stem().context("No file stem")?.to_string_lossy()));
        return Ok(dest);
    }

    if gps_file.file_name().and_then(|n| n.to_str()) == Some("all_activities.gpx") {
        return Ok(gps_file.to_path_buf());
    }

    let file = fs::File::open(gps_file)?;
    let reader = std::io::BufReader::new(file);
    let gpx_data = gpx::read(reader)?;

    let track_name = if let Some(track) = gpx_data.tracks.first() {
        track.name.clone().unwrap_or_else(|| "track".to_string())
    } else {
        "track".to_string()
    };

    let track_time = if let Some(metadata) = gpx_data.metadata {
        if let Some(time) = metadata.time {
            // gpx crate time.format() returns ISO 8601 string Result
            if let Ok(iso) = time.format() {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&iso) {
                    dt.format("%Y-%m-%d.%H:%M:%S").to_string()
                } else {
                    "no_time".to_string()
                }
            } else {
                "no_time".to_string()
            }
        } else {
            "no_time".to_string()
        }
    } else {
        "no_time".to_string()
    };

    let name = format!("{}_{}", track_time, track_name);
    let name = name.replace('/', "-");

    let mut dest = gps_file.parent().unwrap_or(Path::new(".")).to_path_buf();
    dest.push(format!("{}.gpx", name));
    Ok(dest)
}

fn ensure_gpx(gps_file: &Path) -> Result<PathBuf> {
    let dest = gpx_name(gps_file)?;

    let suffix = gps_file.extension().and_then(|s| s.to_str()).unwrap_or("");

    if suffix == "gpx" {
        if gps_file != dest {
            println!("{:?} -> {:?}", gps_file, dest);
            fs::rename(gps_file, &dest)?;
        }
    } else if suffix == "tcx" {
        run("gpsbabel", &[
            "-i", "gtrnctr",
            "-f", gps_file.to_str().context("Path not UTF-8")?,
            "-o", "gpx",
            "-F", dest.to_str().context("Path not UTF-8")?
        ])?;
    } else {
        return Err(anyhow::anyhow!("Unknown format {:?}", suffix));
    }

    Ok(dest)
}

fn merge_gpx(config: &AppConfig, gpx_files: &[PathBuf]) -> Result<PathBuf> {
    let dest = config.images_dir.join("all_activities.gpx");
    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }

    let options = vec!["-i", "gpx"];
    let mut file_args = Vec::new(); // keep strings alive

    for path in gpx_files {
        if path.file_name().and_then(|n| n.to_str()) == Some("all_activities.gpx") {
            continue;
        }
        file_args.push(path.to_string_lossy().to_string());
    }

    let mut cmd_args: Vec<&str> = options;
    for f in &file_args {
        cmd_args.push("-f");
        cmd_args.push(f);
    }
    cmd_args.push("-o");
    cmd_args.push("gpx");
    cmd_args.push("-F");
    let dest_str = dest.to_string_lossy().to_string();
    cmd_args.push(&dest_str);

    run("gpsbabel", &cmd_args)?;

    Ok(dest)
}

fn geotag_images(config: &AppConfig, gpx: &Path) -> Result<()> {
    run("gpicsync", &[
        "-g", gpx.to_str().context("Path not UTF-8")?,
        "-z", "UTC",
        "-d", config.images_dir.to_str().context("Path not UTF-8")?,
        "--time-range", &config.timerange.to_string()
    ])
}

fn clean(config: &AppConfig) -> Result<()> {
    // glob("*_original") in images_dir
    let pattern = config.images_dir.join("*_original");
    for entry in glob::glob(pattern.to_str().context("Path not UTF-8")?)? {
        if let Ok(path) = entry {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn cmd_rename(config: &AppConfig) -> Result<()> {
    let dir_str = config.images_dir.to_string_lossy();

    for suffix in &config.suffixes {
        let up_ext = suffix.to_uppercase();
        let pattern = config.images_dir.join(format!("*.{}", up_ext));

        for entry in glob::glob(pattern.to_str().context("Path not UTF-8")?)? {
            if let Ok(path) = entry {
                let mut new_path = path.clone();
                new_path.set_extension(suffix);

                if path != new_path {
                    println!("Renaming {:?} -> {:?}", path, new_path);
                    if let Err(e) = fs::rename(&path, &new_path) {
                        eprintln!("Failed to rename {:?}: {}", path, e);
                    }
                }
            }
        }
    }

    // chmod
    run("find", &[&dir_str, "-type", "f", "-exec", "chmod", "0644", "{}", "+"])?;

    // exiftool rename
    run("exiftool", &[
        "-FileName<DateTimeOriginal",
        "-d",
        "%Y-%m-%d %H.%M.%S%%-c.%%e",
        "-overwrite_original",
        &dir_str
    ])?;

    clean(config)?;
    Ok(())
}

fn cmd_set_time(config: &AppConfig) -> Result<()> {
    let dst = if !config.timezone_dst { 0 } else { 60 };
    let direction = &config.timezone[0..1];
    let shift = &config.timezone[1..];

    let all_dates_arg = format!("-AllDates{}=0:0:0 {}:0", direction, shift);
    let timezone_arg = format!("-TimeZone={}", config.timezone);
    let timezone_city_arg = format!("-TimeZoneCity#={}", config.timezone_id);
    let offset_time_arg = format!("-OffSetTime={}", config.timezone);
    let offset_time_orig_arg = format!("-OffSetTimeOriginal={}", config.timezone);
    let offset_time_dig_arg = format!("-OffSetTimeDigitized={}", config.timezone);
    let daylight_arg = format!("-DaylightSavings#={}", dst);
    let dir_str = config.images_dir.to_string_lossy();

    run("exiftool", &[
        &all_dates_arg,
        &timezone_arg,
        &timezone_city_arg,
        &offset_time_arg,
        &offset_time_orig_arg,
        &offset_time_dig_arg,
        &daylight_arg,
        "-overwrite_original",
        &dir_str
    ])?;

    clean(config)?;
    Ok(())
}

fn cmd_geotag(config: &AppConfig, gps_files: &[PathBuf]) -> Result<()> {
    if gps_files.is_empty() {
        return Err(anyhow::anyhow!("No gps files provided"));
    }

    let mut gps_paths = Vec::new();
    for path in gps_files {
        gps_paths.push(ensure_gpx(path)?);
    }

    let gpx = if gps_paths.len() > 1 {
        merge_gpx(config, &gps_paths)?
    } else {
        config.images_dir.join(&gps_paths[0])
    };

    geotag_images(config, &gpx)?;
    clean(config)?;
    Ok(())
}

fn cmd_shift(config: &AppConfig, reset_tz: bool, by: &str, images: &[PathBuf]) -> Result<()> {
    if by.is_empty() {
        return Err(anyhow::anyhow!("empty shift pattern"));
    }
    if images.is_empty() {
        return Err(anyhow::anyhow!("No images provided"));
    }

    let (direction, amount) = if by.starts_with('+') || by.starts_with('-') {
        (&by[0..1], &by[1..])
    } else {
        ("+", by)
    };

    let all_dates_arg = format!("-AllDates{}=0:0:0 {}:0", direction, amount);

    let mut args = vec![
        all_dates_arg,
        "-overwrite_original".to_string(),
    ];

    if reset_tz {
        args.push("-OffSetTime=".to_string());
        args.push("-OffSetTimeOriginal=".to_string());
        args.push("-OffSetTimeDigitized=".to_string());
        args.push("-Timezone=".to_string());
        args.push("-TimezoneCity=".to_string());
    }

    let mut final_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let img_strs: Vec<String> = images.iter().map(|p| p.to_string_lossy().to_string()).collect();
    for img in &img_strs {
        final_args.push(img);
    }

    run("exiftool", &final_args)?;
    clean(config)?;
    Ok(())
}

fn cmd_set_gps(config: &AppConfig, lat_ref: &str, long_ref: &str, lat: &str, log: &str, images: &[PathBuf]) -> Result<()> {
    if images.is_empty() {
        return Err(anyhow::anyhow!("No images provided"));
    }

    let _ = lat.trim_end_matches(',').parse::<f64>().context("Invalid lat")?;
    let _ = log.parse::<f64>().context("Invalid log")?;

    let mut r_lat = lat.trim_end_matches(',').to_string();
    let mut r_log = log.to_string();
    let mut r_lat_ref = lat_ref.to_string();
    let mut r_log_ref = long_ref.to_string();

    if r_lat.starts_with('-') {
        r_lat_ref = "S".to_string();
        r_lat = r_lat.trim_start_matches('-').to_string();
    }
    if r_log.starts_with('-') {
        r_log_ref = "W".to_string();
        r_log = r_log.trim_start_matches('-').to_string();
    }

    let mut args = vec![
        format!("-gpslatitude={}", r_lat),
        format!("-gpslongitude={}", r_log),
    ];

    if !r_lat_ref.is_empty() {
        args.push(format!("-gpslatituderef={}", r_lat_ref));
    }
    if !r_log_ref.is_empty() {
        args.push(format!("-gpslongituderef={}", r_log_ref));
    }

    let mut final_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let img_strs: Vec<String> = images.iter().map(|p| p.to_string_lossy().to_string()).collect();
    for img in &img_strs {
        final_args.push(img);
    }

    run("exiftool", &final_args)?;
    clean(config)?;
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let (tz_id, tz_info) = get_tz_info(&cli.timezone)?;

    let config = AppConfig {
        suffixes: cli.suffix.iter().map(|s| s.to_lowercase()).collect(),
        images_dir: cli.images_dir.canonicalize().unwrap_or(cli.images_dir),
        timerange: cli.timerange,
        timezone: tz_info,
        timezone_dst: cli.dst,
        timezone_id: tz_id,
    };

    match &cli.command {
        Commands::Rename => cmd_rename(&config)?,
        Commands::SetTime => cmd_set_time(&config)?,
        Commands::Geotag { gps_files } => cmd_geotag(&config, gps_files)?,
        Commands::Shift { reset_tz, by, images } => cmd_shift(&config, *reset_tz, by, images)?,
        Commands::SetGps { latitude_ref, longitude_ref, lat, log, images } => {
            cmd_set_gps(&config, latitude_ref, longitude_ref, lat, log, images)?
        }
        Commands::All { gps_files } => {
            cmd_geotag(&config, gps_files)?;
            cmd_set_time(&config)?;
            cmd_rename(&config)?;
        }
    }

    Ok(())
}
