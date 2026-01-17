use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

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
    timerange: u64,
    timezone: String,
    timezone_dst: bool,
    timezone_id: i32,
    dry_run: bool,
}

// --- CLI Definitions ---

#[derive(Parser)]
#[command(name = "photo_process")]
#[command(about = "simple scripts to process photos")]
struct Cli {
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
    Rename {
        #[arg(required = true)]
        images: Vec<PathBuf>,
    },
    /// set time and timezone on pictures
    SetTime {
        #[arg(required = true)]
        images: Vec<PathBuf>,
    },
    /// geotag images using gpx files
    Geotag {
        #[arg(short = 'g', long, required = true)]
        gps_files: Vec<PathBuf>,
        #[arg(required = true)]
        images: Vec<PathBuf>,
    },
    /// shift photos - this will also clear out timezones
    Shift {
        #[arg(long, default_value_t = false)]
        reset_tz: bool,
        by: String,
        images: Vec<PathBuf>,
    },
    /// detect timezone from photos and shift to UTC
    ShiftToUtc {
        /// Directories to process
        dirs: Vec<PathBuf>,
    },
    /// detect timezone from photos in directories
    DetectTimezone {
        /// Directories to process
        dirs: Vec<PathBuf>,
    },
    /// Organize photos into directories by date (YYYY-MM-DD)
    Organize {
        /// Directory to organize (defaults to current directory)
        dir: Option<PathBuf>,
    },
    /// Process photos: Shift to UTC, Organize, Geotag, Set Time (with DST), Rename
    Process {
        #[arg(long, default_value_t = false)]
        force: bool,
        /// Directories to process
        dirs: Vec<PathBuf>,
    },
}

// --- Helpers ---

fn run(program: &str, args: &[&str], files: &[&str], dry_run: bool) -> Result<()> {
    if dry_run {
        let mut msg = format!("DRY-RUN: {} {}", program, args.join(" "));
        if !files.is_empty() {
            msg.push(' ');
            msg.push_str(files[0]);
            if files.len() > 1 {
                msg.push_str(&format!(" ... (and {} more files)", files.len() - 1));
            }
        }
        println!("{}", msg.trim());
        return Ok(());
    }

    println!(
        "Running: {} {} {}",
        program,
        args.join(" "),
        files.join(" ")
    );
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

fn cmd_organize(config: &AppConfig, dir: Option<&PathBuf>) -> Result<()> {
    let default_path = PathBuf::from(".");
    let target_dir = dir.unwrap_or(&default_path);

    std::env::set_current_dir(target_dir)
        .with_context(|| format!("Failed to change directory to {:?}", target_dir))?;

    let args = vec![
        "-d",
        "%Y-%m-%d",
        "-Directory<DateTimeOriginal",
        "-ext",
        "jpg",
        "-ext",
        "JPG",
        ".",
    ];

    run("exiftool", &args, &[], config.dry_run)?;
    Ok(())
}

fn organize_files(config: &AppConfig, files: &[PathBuf]) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }

    // Move files to CWD/YYYY-MM-DD/
    // We assume CWD is the target root.

    let args = vec!["-d", "%Y-%m-%d", "-Directory<DateTimeOriginal"];

    let file_strs: Vec<String> = files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let file_refs: Vec<&str> = file_strs.iter().map(|s| s.as_str()).collect();

    run("exiftool", &args, &file_refs, config.dry_run)?;
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
                println!("Renaming extension {:?} -> {:?}", path, new_path);
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
                    println!("DRY-RUN: Rename {:?} to {:?}", path, new_path);
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

fn cmd_rename(config: &AppConfig, images: &[PathBuf]) -> Result<()> {
    let images = fix_extensions(config, images)?;

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

fn cmd_set_time(config: &AppConfig, images: &[PathBuf]) -> Result<()> {
    let images = resolve_files(images)?;

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

fn cmd_geotag(config: &AppConfig, gps_files: &[PathBuf], images: &[PathBuf]) -> Result<()> {
    if gps_files.is_empty() {
        return Err(anyhow::anyhow!("No gps files provided"));
    }
    let images = resolve_files(images)?;
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

fn cmd_shift(config: &AppConfig, reset_tz: bool, by: &str, images: &[PathBuf]) -> Result<()> {
    let images = resolve_files(images)?;
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

fn scan_images_by_dir(config: &AppConfig, dirs: &[PathBuf]) -> HashMap<PathBuf, Vec<PathBuf>> {
    let mut dir_images_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    for dir in dirs {
        if !dir.exists() {
            eprintln!("Warning: Directory {:?} does not exist, skipping.", dir);
            continue;
        }
        let (images, _) = get_files_recursively(dir, config);

        if !images.is_empty() {
            dir_images_map.insert(dir.clone(), images);
        }
    }
    dir_images_map
}

struct TzDetectionResult {
    images: Vec<PathBuf>,
    offset: Result<(String, bool)>,
}

fn detect_timezones(config: &AppConfig, dirs: &[PathBuf]) -> HashMap<PathBuf, TzDetectionResult> {
    let mut results = HashMap::new();
    let dir_images = scan_images_by_dir(config, dirs);

    for (dir, images) in dir_images {
        let offset_res = if let Some(img) = images.first() {
            get_image_offset(img)
        } else {
            Err(anyhow::anyhow!("No images"))
        };

        if let Ok((_, dst)) = &offset_res {
            println!(
                "Directory: {:?}, DST found: {}",
                dir,
                if *dst { "Yes" } else { "No" }
            );
        }

        results.insert(
            dir,
            TzDetectionResult {
                images,
                offset: offset_res,
            },
        );
    }
    results
}

fn cmd_detect_timezone(config: &AppConfig, dirs: &[PathBuf]) -> Result<()> {
    let results = detect_timezones(config, dirs);

    if results.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    for (dir, res) in results {
        match res.offset {
            Ok((offset, _)) => println!("Directory: {:?}, Detected Offset: {}", dir, offset),
            Err(e) => eprintln!("Directory: {:?}, Failed to detect offset: {}", dir, e),
        }
    }
    Ok(())
}

fn cmd_shift_to_utc(config: &AppConfig, dirs: &[PathBuf]) -> Result<()> {
    let results = detect_timezones(config, dirs);

    if results.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    for (dir, res) in results {
        let (offset_str, _) = match res.offset {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Directory: {:?}, Failed to detect offset: {}", dir, e);
                continue;
            }
        };

        println!("Directory: {:?}, Detected Offset: {}", dir, offset_str);
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
        cmd_shift(config, false, &shift_val, &res.images)?;
    }
    Ok(())
}

fn cmd_process(config: &AppConfig, dirs: &[PathBuf]) -> Result<()> {
    // 1. Scan Input Directories
    println!("Scanning input directories for images and GPX files...");

    // We collect all GPX files found for later use
    let mut all_gps_files = Vec::new();
    let mut dir_images_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    for dir in dirs {
        if !dir.exists() {
            eprintln!("Warning: Directory {:?} does not exist, skipping.", dir);
            continue;
        }
        let (images, gpx) = get_files_recursively(dir, config);
        all_gps_files.extend(gpx);

        if !images.is_empty() {
            dir_images_map.insert(dir.clone(), images);
        }
    }

    // 2. Shift to UTC
    cmd_shift_to_utc(config, dirs)?;

    // 3. Organize (Move to CWD)
    if !dir_images_map.is_empty() {
        for images in dir_images_map.values() {
            println!("  -> Organizing (Moving to CWD)");
            organize_files(config, images)?;
        }
    }

    // 4. Scan CWD for processing (Geotag, SetTime, Rename)
    // Files have moved to ./YYYY-MM-DD/
    // We scan CWD recursively.

    if config.dry_run {
        println!("---");
        println!("Plan complete.");
        println!("Subsequent steps (Geotag, SetTime, Rename) would run on organized files in current directory.");
        println!("Run with --force to execute.");
        return Ok(());
    }

    // FORCE MODE: Scan and Process
    let cwd = std::env::current_dir()?;
    let (cwd_images, cwd_gpx) = get_files_recursively(&cwd, config);

    // Add any GPX found in input dirs (all_gps_files) to cwd_gpx?
    // User might have GPX in input dirs that didn't move (Organize moves images only).
    // So we should combine them.
    let mut final_gps_files = all_gps_files;
    final_gps_files.extend(cwd_gpx);

    // Deduplicate GPX
    final_gps_files.sort();
    final_gps_files.dedup();

    println!(
        "Found {} images and {} GPX files in workspace.",
        cwd_images.len(),
        final_gps_files.len()
    );

    // 5. Geotag
    if !final_gps_files.is_empty() {
        cmd_geotag(config, &final_gps_files, &cwd_images)?;
    } else {
        println!("No GPX files found, skipping geotag.");
    }

    // 6. Set Time (UTC -> Target)
    // We want to shift from UTC to config.timezone.
    // existing cmd_set_time does this (AllDates += config.timezone).
    println!("Setting time and timezone to {}", config.timezone);
    cmd_set_time(config, &cwd_images)?;

    // 7. Rename
    cmd_rename(config, &cwd_images)?;

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let (tz_id, tz_info) = get_tz_info(&cli.timezone)?;

    let dry_run = match &cli.command {
        Commands::Process { force, .. } => !*force,
        _ => false,
    };

    let config = AppConfig {
        suffixes: cli.suffix.iter().map(|s| s.to_lowercase()).collect(),
        timerange: cli.timerange,
        timezone: tz_info,
        timezone_dst: cli.dst,
        timezone_id: tz_id,
        dry_run,
    };

    match &cli.command {
        Commands::Rename { images } => cmd_rename(&config, images)?,
        Commands::SetTime { images } => cmd_set_time(&config, images)?,
        Commands::Geotag { gps_files, images } => cmd_geotag(&config, gps_files, images)?,
        Commands::Shift {
            reset_tz,
            by,
            images,
        } => cmd_shift(&config, *reset_tz, by, images)?,
        Commands::ShiftToUtc { dirs } => cmd_shift_to_utc(&config, dirs)?,
        Commands::DetectTimezone { dirs } => cmd_detect_timezone(&config, dirs)?,
        Commands::Organize { dir } => cmd_organize(&config, dir.as_ref())?,
        Commands::Process { dirs, .. } => cmd_process(&config, dirs)?,
    }

    Ok(())
}
