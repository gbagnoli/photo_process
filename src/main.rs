use anyhow::{Context, Result};
use chrono::{Duration, Local, NaiveDate};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

// --- Constants & Config ---

/* not an exaustive list
see https://sno.phy.queensu.ca/~phil/exiftool/TagNames/Canon.html
https://github.com/alchemy-fr/exiftool/blob/master/lib/Image/ExifTool/Canon.pm#L5891-L5930
*/
const TZ_CITIES_DATA: &[(&str, i32, &str)] = &[
    ("Adelaide", 5, "+09:30"),
    ("Anchorage", 31, "-09:00"),
    ("Austin", 28, "-06:00"),
    ("Azores", 21, "-01:00"),
    ("Bangkok", 8, "+07:00"),
    ("Buenos Aires", 25, "-04:00"),
    ("Cairo", 18, "+02:00"),
    ("Caracas", 26, "-04:30"),
    ("Chatham Islands", 1, "+12:45"),
    ("Chicago", 28, "-06:00"),
    ("Delhi", 12, "+05:30"),
    ("Denver", 29, "-07:00"),
    ("Dhaka", 10, "+06:00"),
    ("Dubai", 15, "+04:00"),
    ("Dublin", 20, "+00:00"),
    ("Fernando de Noronha", 22, "-02:00"),
    ("Galapagos", 28, "-06:00"),
    ("Hong Kong", 7, "+08:00"),
    ("Honolulu", 32, "-10:00"),
    ("Kabul", 14, "+04:30"),
    ("Karachi", 13, "+05:00"),
    ("Kathmandu", 11, "+05:45"),
    ("Kiev", 17, "+02:00"),
    ("London", 20, "+00:00"),
    ("Los Angeles", 30, "-08:00"),
    ("Mexico City", 28, "-06:00"),
    ("Moscow", 17, "+04:00"),
    ("New York", 27, "-05:00"),
    ("Newfoundland", 24, "-03:30"),
    ("Paris", 19, "+01:00"),
    ("Quintana Roo", 27, "-05:00"),
    ("Quito", 27, "-05:00"),
    ("Rome", 19, "+01:00"),
    ("Samoa", 33, "+13:00"),
    ("San Francisco", 30, "-08:00"),
    ("Santiago", 25, "-04:00"),
    ("Sao Paulo", 23, "-03:00"),
    ("Singapore", 7, "+08:00"),
    ("Solomon Islands", 3, "+11:00"),
    ("Sydney", 4, "+10:00"),
    ("Tehran", 16, "+03:30"),
    ("Tokyo", 6, "+09:00"),
    ("US/Central", 28, "-06:00"),
    ("US/Eastern", 27, "-05:00"),
    ("US/Pacific", 30, "-08:00"),
    ("Wellington", 2, "+12:00"),
    ("Yangon", 9, "+06:30"),
];

#[derive(Debug, Clone)]
struct AppConfig {
    suffixes: Vec<String>,
    timerange: u64,
    dry_run: bool,
}

// --- CLI Definitions ---

#[derive(Parser)]
#[command(name = "photo_process")]
#[command(about = "simple scripts to process photos")]
struct Cli {
    #[arg(long, default_value_t = 10)]
    timerange: u64,

    #[arg(
        short = 'e',
        long,
        default_value = "jpg,JPG,mp4",
        value_delimiter = ','
    )]
    suffix: Vec<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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

// --- Helpers ---

fn run(program: &str, args: &[&str], files: &[&str], dry_run: bool) -> Result<()> {
    let mut msg = if dry_run {
        format!("DRY-RUN: {} {}", program, args.join(" "))
    } else {
        format!("Running: {} {}", program, args.join(" "))
    };

    if !files.is_empty() {
        msg.push(' ');
        msg.push_str(files[0]);
        if files.len() > 1 {
            msg.push_str(&format!(" ... (and {} more files)", files.len() - 1));
        }
    }
    println!("{}", msg.trim());

    if dry_run {
        return Ok(());
    }

    let status = Command::new(program)
        .args(args)
        .args(files)
        .status()
        .with_context(|| format!("Failed to execute {}", program))?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Command {} exited with non-zero status",
            program
        ));
    }
    Ok(())
}

fn run_capture(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("Failed to execute {}", program))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Command {} failed", program));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn resolve_files(files: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut resolved = Vec::new();
    for path in files {
        if path.exists() {
            resolved.push(
                fs::canonicalize(path)
                    .with_context(|| format!("Failed to canonicalize {:?}", path))?,
            );
        } else {
            return Err(anyhow::anyhow!("File not found: {:?}", path));
        }
    }
    Ok(resolved)
}

fn get_files_recursively(dir: &Path, config: &AppConfig) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut images = Vec::new();
    let mut gpx_files = Vec::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if ext == "gpx" {
                gpx_files.push(path.to_path_buf());
            } else if config.suffixes.contains(&ext) {
                images.push(path.to_path_buf());
            }
        }
    }
    (images, gpx_files)
}

fn get_tz_info(city: &str) -> Result<(i32, String)> {
    for (name, id, offset) in TZ_CITIES_DATA {
        if *name == city {
            return Ok((*id, offset.to_string()));
        }
    }
    Err(anyhow::anyhow!("Unknown timezone city: {}", city))
}

/// Returns a list of city names that match the given offset (e.g., "+01:00")
#[allow(dead_code)]
fn get_cities_by_offset(offset: &str) -> Vec<&str> {
    TZ_CITIES_DATA
        .iter()
        .filter(|(_, _, tz_offset)| *tz_offset == offset)
        .map(|(name, _, _)| *name)
        .collect()
}

/// Builds a full reverse index mapping offsets to lists of city names
#[allow(dead_code)]
fn get_reverse_timezone_index() -> HashMap<String, Vec<&'static str>> {
    let mut index: HashMap<String, Vec<&'static str>> = HashMap::new();

    for (name, _, offset) in TZ_CITIES_DATA {
        index.entry(offset.to_string()).or_default().push(name);
    }

    index
}

fn gpx_name(gps_file: &Path, _dry_run: bool) -> Result<PathBuf> {
    if gps_file.extension().and_then(|e| e.to_str()) != Some("gpx") {
        let mut dest = gps_file.parent().unwrap_or(Path::new(".")).to_path_buf();
        dest.push(format!(
            "{}.gpx",
            gps_file
                .file_stem()
                .context("No file stem")?
                .to_string_lossy()
        ));
        return Ok(dest);
    }

    if gps_file.file_name().and_then(|n| n.to_str()) == Some("all_activities.gpx") {
        return Ok(gps_file.to_path_buf());
    }

    // In dry run, we might not be able to read file if it doesn't exist yet (created by previous step?)
    // But here we read existing files.

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

fn ensure_gpx(gps_file: &Path, dry_run: bool) -> Result<PathBuf> {
    let dest = gpx_name(gps_file, dry_run)?;

    let suffix = gps_file.extension().and_then(|s| s.to_str()).unwrap_or("");

    if suffix == "gpx" {
        if gps_file != dest {
            println!("{:?} -> {:?}", gps_file, dest);
            if !dry_run {
                fs::rename(gps_file, &dest)?;
            }
        }
    } else {
        return Err(anyhow::anyhow!(
            "Unknown format {:?}. Only .gpx is supported.",
            suffix
        ));
    }

    Ok(dest)
}

fn merge_gpx(gpx_files: &[PathBuf], output_dir: &Path, dry_run: bool) -> Result<PathBuf> {
    let dest = output_dir.join("all_activities.gpx");
    if dry_run {
        println!(
            "DRY-RUN: Merge {} GPX files into {:?}",
            gpx_files.len(),
            dest
        );
        return Ok(dest);
    }

    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }

    let mut merged_gpx = gpx::Gpx {
        version: gpx::GpxVersion::Gpx11,
        ..Default::default()
    };

    for path in gpx_files {
        if path.file_name().and_then(|n| n.to_str()) == Some("all_activities.gpx") {
            continue;
        }
        let file = fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let g = gpx::read(reader).with_context(|| format!("Failed to read GPX file {:?}", path))?;

        merged_gpx.tracks.extend(g.tracks);
        merged_gpx.routes.extend(g.routes);
        merged_gpx.waypoints.extend(g.waypoints);
    }

    let file = fs::File::create(&dest)?;
    let writer = std::io::BufWriter::new(file);
    gpx::write(&merged_gpx, writer)
        .with_context(|| format!("Failed to write merged GPX to {:?}", dest))?;

    Ok(dest)
}

fn geotag_images_dir(config: &AppConfig, gpx: &Path, dir: &Path) -> Result<()> {
    run(
        "gpicsync",
        &[
            "-g",
            gpx.to_str().context("Path not UTF-8")?,
            "-z",
            "UTC",
            "-d",
            dir.to_str().context("Path not UTF-8")?,
            "--time-range",
            &config.timerange.to_string(),
        ],
        &[],
        config.dry_run,
    )
}

fn clean(files: &[PathBuf], dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    for path in files {
        let mut original = path.clone();
        if let Some(name) = path.file_name() {
            let mut name_str = name.to_string_lossy().into_owned();
            name_str.push_str("_original");
            original.set_file_name(name_str);

            if original.exists() {
                let _ = fs::remove_file(original);
            }
        }
    }
    Ok(())
}

fn remove_empty_dirs_recursive(dir: &Path, dry_run: bool) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            remove_empty_dirs_recursive(&path, dry_run)?;
            if fs::read_dir(&path)?.next().is_none() {
                if dry_run {
                    println!("DRY-RUN: Removing empty directory: {:?}", path);
                } else {
                    println!("Removing empty directory: {:?}", path);
                    fs::remove_dir(&path)?;
                }
            }
        }
    }
    Ok(())
}

fn parse_offset(s: &str) -> Result<i32> {
    let s = s.trim();
    if s.is_empty() {
        return Err(anyhow::anyhow!("Empty offset"));
    }
    let sign = if s.starts_with('-') { -1 } else { 1 };
    let s = s.trim_start_matches('+').trim_start_matches('-');
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 {
        return Err(anyhow::anyhow!("Invalid offset format: {}", s));
    }
    let h: i32 = parts[0].parse()?;
    let m: i32 = parts[1].parse()?;
    Ok(sign * (h * 60 + m))
}

fn format_offset(mins: i32) -> String {
    let sign = if mins >= 0 { "+" } else { "-" };
    let abs_mins = mins.abs();
    let h = abs_mins / 60;
    let m = abs_mins % 60;
    format!("{}{:02}:{:02}", sign, h, m)
}

fn get_image_offset(file: &Path) -> Result<(String, bool)> {
    let args = &[
        "-G1",
        "-a",
        "-s",
        "-DateTimeOriginal",
        "-DaylightSavings",
        "-TimeZone",
        "-OffsetTimeOriginal",
        file.to_str().unwrap(),
    ];
    println!("Running: exiftool {}", args.join(" "));
    let output = run_capture("exiftool", args)?;

    let mut dst_on = false;
    let mut offset_time_orig: Option<String> = None;
    let mut timezone_val: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Line format: [Group] Tag : Value
        // We look for the tag name part.
        // Split by ':'
        if let Some((key_part, value_part)) = line.split_once(':') {
            let key = key_part.trim(); // e.g. "[ExifIFD] OffsetTimeOriginal"
            let value = value_part.trim();

            // Extract Tag name from [Group] Tag
            let tag = if let Some((_, t)) = key.rsplit_once(' ') {
                t
            } else {
                key // fallback if no group?
            };

            match tag {
                "DaylightSavings" => {
                    if value == "On" {
                        dst_on = true;
                    }
                }
                "OffsetTimeOriginal" => {
                    offset_time_orig = Some(value.to_string());
                }
                "TimeZone" => {
                    timezone_val = Some(value.to_string());
                }
                _ => {}
            }
        }
    }

    if let Some(oto) = offset_time_orig {
        // If OffsetTimeOriginal exists, use it directly.
        // Usually it includes DST if applicable.
        return Ok((oto, dst_on));
    }

    if let Some(tz) = timezone_val {
        // Fallback to TimeZone.
        // Adjust for DST if needed.
        let mut mins = parse_offset(&tz)?;
        if dst_on {
            mins += 60;
        }
        return Ok((format_offset(mins), dst_on));
    }

    Err(anyhow::anyhow!("No offset found in {:?}", file))
}

// --- Commands ---

fn cmd_organize(config: &AppConfig, dirs: &[PathBuf]) -> Result<()> {
    for dir in dirs {
        if !dir.exists() {
            return Err(anyhow::anyhow!("Directory does not exist: {:?}", dir));
        }
        let (images, _) = get_files_recursively(dir, config);
        if images.is_empty() {
            println!("No images found in {:?}", dir);
            continue;
        }

        let abs_dir = fs::canonicalize(dir)?;
        let abs_dir_str = abs_dir.to_str().context("Path not UTF-8")?;

        // We want to move every image under `dir` to `dir/YYYY-MM-DD/`
        let dir_target = format!("-Directory<{}/$DateTimeOriginal", abs_dir_str);
        let args = vec!["-d", "%Y-%m-%d", &dir_target];

        let file_strs: Vec<String> = images
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let file_refs: Vec<&str> = file_strs.iter().map(|s| s.as_str()).collect();

        run("exiftool", &args, &file_refs, config.dry_run)?;

        remove_empty_dirs_recursive(dir, config.dry_run)?;
    }
    Ok(())
}

fn fix_extensions(config: &AppConfig, files: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut resolved = Vec::new();
    let files = resolve_files(files)?;

    for path in files {
        let suffix = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if config.suffixes.contains(&suffix) {
            let mut new_path = path.clone();
            new_path.set_extension(&suffix); // force lowercase suffix

            if path != new_path {
                if !config.dry_run {
                    if let Err(e) = fs::rename(&path, &new_path) {
                        eprintln!("Failed to rename {:?}: {}", path, e);
                        if new_path.exists() {
                            resolved.push(new_path);
                        } else {
                            resolved.push(path);
                        }
                    } else {
                        resolved.push(new_path);
                    }
                } else {
                    resolved.push(new_path); // assume successful for dry-run flow logic?
                }
            } else {
                resolved.push(path);
            }
        } else {
            resolved.push(path);
        }
    }
    Ok(resolved)
}

fn cmd_rename(config: &AppConfig, paths: &[PathBuf]) -> Result<()> {
    let images = get_all_images_from_paths(config, paths);
    let images = fix_extensions(config, &images)?;

    let img_strs: Vec<String> = images
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let img_refs: Vec<&str> = img_strs.iter().map(|s| s.as_str()).collect();

    run("chmod", &["0644"], &img_refs, config.dry_run)?;

    let exif_opts = vec![
        "-FileName<DateTimeOriginal",
        "-d",
        "%Y-%m-%d %H.%M.%S%%-c.%%e",
        "-overwrite_original",
    ];

    run("exiftool", &exif_opts, &img_refs, config.dry_run)?;

    clean(&images, config.dry_run)?;
    Ok(())
}

fn cmd_set_time(
    config: &AppConfig,
    paths: &[PathBuf],
    _set_gps: bool,
    timezone: &str,
    timezone_id: i32,
    dst: bool,
) -> Result<()> {
    let images = get_all_images_from_paths(config, paths);
    let images = resolve_files(&images)?;

    let dst_val = if !dst { 0 } else { 60 };
    let direction = &timezone[0..1];
    let shift = &timezone[1..];

    let all_dates_arg = format!("-AllDates{}=0:0:0 {}:0", direction, shift);
    let timezone_arg = format!("-TimeZone={}", timezone);
    let timezone_city_arg = format!("-TimeZoneCity#={}", timezone_id);
    let offset_time_arg = format!("-OffSetTime={}", timezone);
    let offset_time_orig_arg = format!("-OffSetTimeOriginal={}", timezone);
    let offset_time_dig_arg = format!("-OffSetTimeDigitized={}", timezone);
    let daylight_arg = format!("-DaylightSavings#={}", dst_val);

    let args = vec![
        all_dates_arg.as_str(),
        timezone_arg.as_str(),
        timezone_city_arg.as_str(),
        offset_time_arg.as_str(),
        offset_time_orig_arg.as_str(),
        offset_time_dig_arg.as_str(),
        daylight_arg.as_str(),
        "-overwrite_original",
    ];

    let img_strs: Vec<String> = images
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let img_refs: Vec<&str> = img_strs.iter().map(|s| s.as_str()).collect();

    run("exiftool", &args, &img_refs, config.dry_run)?;
    Ok(())
}

fn cmd_geotag(config: &AppConfig, gps_files: &[PathBuf], paths: &[PathBuf]) -> Result<()> {
    if gps_files.is_empty() {
        return Err(anyhow::anyhow!("No gps files provided"));
    }
    let images = get_all_images_from_paths(config, paths);
    let images = resolve_files(&images)?;
    let gps_files = resolve_files(gps_files)?;

    let mut gps_paths = Vec::new();
    for path in gps_files {
        gps_paths.push(ensure_gpx(&path, config.dry_run)?);
    }

    let mut dirs: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for img in &images {
        if let Some(parent) = img.parent() {
            dirs.entry(parent.to_path_buf())
                .or_default()
                .push(img.clone());
        }
    }

    for (dir, _) in dirs {
        println!("Processing directory: {:?}", dir);

        let gpx = if gps_paths.len() > 1 {
            merge_gpx(&gps_paths, &dir, config.dry_run)?
        } else {
            gps_paths[0].clone()
        };

        geotag_images_dir(config, &gpx, &dir)?;

        if gps_paths.len() > 1 && gpx.exists() && !config.dry_run {
            if let Err(e) = fs::remove_file(&gpx) {
                eprintln!("Failed to remove temporary gpx {:?}: {}", gpx, e);
            }
        }
    }

    clean(&images, config.dry_run)?;
    Ok(())
}

fn cmd_shift(config: &AppConfig, reset_tz: bool, by: &str, paths: &[PathBuf]) -> Result<()> {
    let images = get_all_images_from_paths(config, paths);
    let images = resolve_files(&images)?;
    if by.is_empty() {
        return Err(anyhow::anyhow!("empty shift pattern"));
    }

    let (direction, amount) = if by.starts_with('+') || by.starts_with('-') {
        (&by[0..1], &by[1..])
    } else {
        ("+", by)
    };

    let all_dates_arg = format!("-AllDates{}=0:0:0 {}:0", direction, amount);

    let mut args = vec![all_dates_arg.as_str(), "-overwrite_original"];

    if reset_tz {
        args.push("-OffSetTime=");
        args.push("-OffSetTimeOriginal=");
        args.push("-OffSetTimeDigitized=");
        args.push("-Timezone=");
        args.push("-TimezoneCity=");
    }

    let img_strs: Vec<String> = images
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let img_refs: Vec<&str> = img_strs.iter().map(|s| s.as_str()).collect();

    run("exiftool", &args, &img_refs, config.dry_run)?;
    Ok(())
}

fn scan_images_from_paths(config: &AppConfig, paths: &[PathBuf]) -> HashMap<PathBuf, Vec<PathBuf>> {
    let mut dir_images_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    for path in paths {
        if !path.exists() {
            eprintln!("Warning: Path {:?} does not exist, skipping.", path);
            continue;
        }
        let (images, _) = get_files_recursively(path, config);

        if !images.is_empty() {
            dir_images_map.insert(path.clone(), images);
        }
    }
    dir_images_map
}

fn get_all_images_from_paths(config: &AppConfig, paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut all_images = Vec::new();
    for path in paths {
        if path.exists() {
            let (images, _) = get_files_recursively(path, config);
            all_images.extend(images);
        } else {
            eprintln!("Warning: Path {:?} does not exist, skipping.", path);
        }
    }
    all_images
}

struct TzDetectionResult {
    images: Vec<PathBuf>,
    offset: Result<(String, bool)>,
}

fn detect_timezones(config: &AppConfig, paths: &[PathBuf]) -> HashMap<PathBuf, TzDetectionResult> {
    let mut results = HashMap::new();
    let dir_images = scan_images_from_paths(config, paths);

    for (path, images) in dir_images {
        let offset_res = if let Some(img) = images.first() {
            get_image_offset(img)
        } else {
            Err(anyhow::anyhow!("No images"))
        };

        results.insert(
            path,
            TzDetectionResult {
                images,
                offset: offset_res,
            },
        );
    }
    results
}

fn cmd_detect_timezone(config: &AppConfig, paths: &[PathBuf]) -> Result<()> {
    let results = detect_timezones(config, paths);

    if results.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    for (path, res) in results {
        let label = if path.is_dir() { "Directory" } else { "File" };
        match res.offset {
            Ok((offset, dst)) => {
                println!(
                    "{}: {:?}, Detected Offset: {}, DST found: {}",
                    label,
                    path,
                    offset,
                    if dst { "Yes" } else { "No" }
                );
            }
            Err(e) => eprintln!("{}: {:?}, Failed to detect offset: {}", label, path, e),
        }
    }
    Ok(())
}

fn cmd_shift_to_utc(config: &AppConfig, paths: &[PathBuf]) -> Result<()> {
    let results = detect_timezones(config, paths);

    if results.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    for (path, res) in results {
        let label = if path.is_dir() { "Directory" } else { "File" };
        let (offset_str, dst) = match res.offset {
            Ok(o) => o,
            Err(e) => {
                eprintln!("{}: {:?}, Failed to detect offset: {}", label, path, e);
                continue;
            }
        };

        println!(
            "{}: {:?}, Detected Offset: {}, DST found: {}",
            label,
            path,
            offset_str,
            if dst { "Yes" } else { "No" }
        );
        // Parse offset
        // format: +HH:MM or -HH:MM
        let (sign, rest) = if offset_str.starts_with('+') || offset_str.starts_with('-') {
            (&offset_str[0..1], &offset_str[1..])
        } else {
            ("+", offset_str.as_str())
        };

        // Parse HH:MM
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() < 2 {
            eprintln!("Invalid offset format {}, skipping shift.", offset_str);
            continue;
        }

        let shift_sign = if sign == "+" { "-" } else { "+" };
        let shift_val = format!("{}{}:{}", shift_sign, parts[0], parts[1]);

        println!("  -> Shifting to UTC by {}", shift_val);
        cmd_shift(config, true, &shift_val, &res.images)?;
    }
    Ok(())
}

fn cmd_process(
    config: &AppConfig,
    dirs: &[PathBuf],
    timezone: &str,
    timezone_id: i32,
    dst: bool,
    organize: bool,
) -> Result<()> {
    // 1. Scan and Detect Timezones
    println!("Scanning input directories for images and GPX files...");
    let results = detect_timezones(config, dirs);

    if results.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    let mut dir_images_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    for (path, res) in &results {
        dir_images_map.insert(path.clone(), res.images.clone());
    }

    // 2. Shift to UTC
    for (path, res) in results {
        let (offset_str, dst_found) = match res.offset {
            Ok(o) => o,
            Err(e) => {
                eprintln!(
                    "{}: {:?}, Failed to detect offset: {}",
                    if path.is_dir() { "Directory" } else { "File" },
                    path,
                    e
                );
                continue;
            }
        };

        println!(
            "{}: {:?}, Detected Offset: {}, DST found: {}",
            if path.is_dir() { "Directory" } else { "File" },
            path,
            offset_str,
            if dst_found { "Yes" } else { "No" }
        );

        let (sign, rest) = if offset_str.starts_with('+') || offset_str.starts_with('-') {
            (&offset_str[0..1], &offset_str[1..])
        } else {
            ("+", offset_str.as_str())
        };

        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() < 2 {
            continue;
        }

        let shift_sign = if sign == "+" { "-" } else { "+" };
        let shift_val = format!("{}{}:{}", shift_sign, parts[0], parts[1]);

        println!("  -> Shifting to UTC by {}", shift_val);
        cmd_shift(config, false, &shift_val, &res.images)?;
    }

    // 3. Organize & Download GPX
    if organize {
        println!("  -> Organizing photos...");
        cmd_organize(config, dirs)?;

        // Determine date range from organized folders
        let mut min_date: Option<NaiveDate> = None;
        let mut max_date: Option<NaiveDate> = None;
        let date_re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$")?;

        for dir in dirs {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if date_re.is_match(&name) {
                        if let Ok(date) = NaiveDate::parse_from_str(&name, "%Y-%m-%d") {
                            if min_date.is_none() || date < min_date.unwrap() {
                                min_date = Some(date);
                            }
                            if max_date.is_none() || date > max_date.unwrap() {
                                max_date = Some(date);
                            }
                        }
                    }
                }
            }
        }

        if let (Some(start), Some(end)) = (min_date, max_date) {
            let start_str = start.format("%Y-%m-%d").to_string();
            let end_str = end.format("%Y-%m-%d").to_string();
            println!("  -> Detected date range: {} to {}", start_str, end_str);

            for dir in dirs {
                println!("  -> Downloading GPX files to {:?}", dir);
                cmd_download_gpx(config, dir, Some(&start_str), Some(&end_str))?;
            }
        }
    }

    // 5. Re-scan for processing (images and downloaded GPX)
    let mut all_images = Vec::new();
    let mut all_gpx = Vec::new();

    for dir in dirs {
        let (imgs, gpxs) = get_files_recursively(dir, config);
        all_images.extend(imgs);
        all_gpx.extend(gpxs);
    }

    if all_images.is_empty() {
        println!("No images found after organization, finishing.");
        return Ok(());
    }

    // 6. Geotag
    if !all_gpx.is_empty() {
        cmd_geotag(config, &all_gpx, &all_images)?;
    } else {
        println!("No GPX files found, skipping geotag.");
    }

    // 7. Set Time (UTC -> Target)
    println!("Setting time and timezone to {}", timezone);
    cmd_set_time(config, &all_images, true, timezone, timezone_id, dst)?;

    // 8. Rename
    cmd_rename(config, &all_images)?;

    Ok(())
}

fn cmd_download_gpx(
    _config: &AppConfig,
    dest: &Path,
    start_date: Option<&String>,
    end_date: Option<&String>,
) -> Result<()> {
    let end = match end_date {
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")?,
        None => Local::now().date_naive(),
    };
    let start = match start_date {
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")?,
        None => end - Duration::days(20),
    };

    println!("Downloading activities from {} to {}", start, end);

    if !dest.exists() {
        fs::create_dir_all(dest)?;
    }

    let mut offset = 0;
    let limit = 100;

    loop {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        let output = run_capture(
            "garmin",
            &[
                "activities",
                "list",
                "--limit",
                &limit_str,
                "--start",
                &offset_str,
            ],
        )?;

        let mut found_any = false;
        let mut all_older = true;

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("ID") || line.starts_with('-') {
                continue;
            }

            // Split by whitespace
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            found_any = true;
            let activity_id = parts[0];
            let activity_date_str = parts[1];

            let activity_date = match NaiveDate::parse_from_str(activity_date_str, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };

            if activity_date < start {
                continue;
            }

            all_older = false;

            if activity_date <= end {
                let gpx_filename = format!("{}.gpx", activity_id);
                let gpx_path = dest.join(gpx_filename);

                if gpx_path.exists() {
                    println!("Activity {} already downloaded, checking name...", activity_id);
                    let _ = ensure_gpx(&gpx_path, false)?;
                    continue;
                }

                println!(
                    "Downloading activity {} ({})...",
                    activity_id, activity_date_str
                );
                let gpx_path_str = gpx_path.to_string_lossy().to_string();

                run(
                    "garmin",
                    &[
                        "activities",
                        "download",
                        "-t",
                        "gpx",
                        "-o",
                        &gpx_path_str,
                        activity_id,
                    ],
                    &[],
                    false,
                )?;

                // Rename the downloaded GPX file using its track name and time
                let _ = ensure_gpx(&gpx_path, false)?;
            }
        }

        if !found_any || all_older {
            break;
        }
        offset += limit;
    }

    // After all downloads and renames, merge everything into all_activities.gpx
    let (_, gpx_files) = get_files_recursively(dest, _config);
    if !gpx_files.is_empty() {
        let _ = merge_gpx(&gpx_files, dest, false)?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let dry_run = match &cli.command {
        Commands::Process { force, .. } => !*force,
        _ => false,
    };

    let config = AppConfig {
        suffixes: cli.suffix.iter().map(|s| s.to_lowercase()).collect(),
        timerange: cli.timerange,
        dry_run,
    };

    match &cli.command {
        Commands::Rename { paths } => cmd_rename(&config, paths)?,
        Commands::SetTime {
            paths,
            timezone,
            dst,
        } => {
            let (tz_id, tz_info) = get_tz_info(timezone)?;
            cmd_set_time(&config, paths, false, &tz_info, tz_id, *dst)?
        }
        Commands::Geotag { gps_files, paths } => cmd_geotag(&config, gps_files, paths)?,
        Commands::Shift {
            reset_tz,
            by,
            paths,
        } => cmd_shift(&config, *reset_tz, by, paths)?,
        Commands::ShiftToUtc { paths } => cmd_shift_to_utc(&config, paths)?,
        Commands::DetectTimezone { paths } => cmd_detect_timezone(&config, paths)?,
        Commands::Organize { dirs } => cmd_organize(&config, dirs)?,
        Commands::Process {
            dirs,
            timezone,
            dst,
            organize,
            ..
        } => {
            let (tz_id, tz_info) = get_tz_info(timezone)?;
            cmd_process(
                &config,
                dirs,
                &tz_info,
                tz_id,
                *dst,
                *organize,
            )?
        }
        Commands::DownloadGpx {
            dest,
            start_date,
            end_date,
        } => {
            cmd_download_gpx(&config, dest, start_date.as_ref(), end_date.as_ref())?;
        }
    }

    Ok(())
}