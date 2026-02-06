use anyhow::{Context, Result};
use colored::Colorize;
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
pub const TZ_CITIES_DATA: &[(&str, i32, &str)] = &[
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
pub struct AppConfig {
    pub suffixes: Vec<String>,
    pub timerange: u64,
    pub dry_run: bool,
}

// --- Helpers ---

pub fn run(program: &str, args: &[&str], files: &[&str], dry_run: bool) -> Result<()> {
    let mut cmd_str = format!("{} {}", program, args.join(" "));

    if !files.is_empty() {
        cmd_str.push(' ');
        cmd_str.push_str(files[0]);
        if files.len() > 1 {
            cmd_str.push_str(&format!(" ... (and {} more files)", files.len() - 1));
        }
    }

    if dry_run {
        println!("{}", format!("DRY-RUN: {}", cmd_str).green());
        return Ok(());
    }

    println!("{}", cmd_str.cyan().bold());

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

pub fn run_capture(program: &str, args: &[&str]) -> Result<String> {
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

pub fn resolve_files(files: &[PathBuf]) -> Result<Vec<PathBuf>> {
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

pub fn get_files_recursively(dir: &Path, config: &AppConfig) -> (Vec<PathBuf>, Vec<PathBuf>) {
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

pub fn get_tz_info(city: &str) -> Result<(i32, String)> {
    for (name, id, offset) in TZ_CITIES_DATA {
        if *name == city {
            return Ok((*id, offset.to_string()));
        }
    }
    Err(anyhow::anyhow!("Unknown timezone city: {}", city))
}

pub fn gpx_name(gps_file: &Path, _dry_run: bool) -> Result<PathBuf> {
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
                    dt.format("%Y-%m-%d.%H.%M.%S").to_string()
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

pub fn ensure_gpx(gps_file: &Path, dry_run: bool) -> Result<PathBuf> {
    if !gps_file.exists() {
        if dry_run {
            println!(
                "{}",
                format!("DRY-RUN: Would rename {:?} based on track name", gps_file).green()
            );
            return Ok(gps_file.to_path_buf());
        }
        return Err(anyhow::anyhow!("File not found: {:?}", gps_file));
    }
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

pub fn merge_gpx(gpx_files: &[PathBuf], output_dir: &Path, dry_run: bool) -> Result<PathBuf> {
    let dest = output_dir.join("all_activities.gpx");
    if dry_run {
        println!(
            "{}",
            format!(
                "DRY-RUN: Merge {} GPX files into {:?}",
                gpx_files.len(),
                dest
            )
            .green()
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

pub fn clean(files: &[PathBuf], dry_run: bool) -> Result<()> {
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

pub fn fix_extensions(config: &AppConfig, files: &[PathBuf]) -> Result<Vec<PathBuf>> {
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

pub fn get_all_images_from_paths(config: &AppConfig, paths: &[PathBuf]) -> Vec<PathBuf> {
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

pub struct TzDetectionResult {
    pub images: Vec<PathBuf>,
    pub offset: Result<(String, bool)>,
}

pub fn scan_images_from_paths(
    config: &AppConfig,
    paths: &[PathBuf],
) -> HashMap<PathBuf, Vec<PathBuf>> {
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

pub fn detect_timezones(
    config: &AppConfig,
    paths: &[PathBuf],
) -> HashMap<PathBuf, TzDetectionResult> {
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

pub fn cmd_shift(config: &AppConfig, reset_tz: bool, by: &str, paths: &[PathBuf]) -> Result<()> {
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

pub fn remove_empty_dirs_recursive(dir: &Path, dry_run: bool) -> Result<()> {
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
                    println!(
                        "{}",
                        format!("DRY-RUN: Removing empty directory: {:?}", path).green()
                    );
                } else {
                    println!("Removing empty directory: {:?}", path);
                    fs::remove_dir(&path)?;
                }
            }
        }
    }
    Ok(())
}

pub fn parse_offset(s: &str) -> Result<i32> {
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

pub fn format_offset(mins: i32) -> String {
    let sign = if mins >= 0 { "+" } else { "-" };
    let abs_mins = mins.abs();
    let h = abs_mins / 60;
    let m = abs_mins % 60;
    format!("{}{:02}:{:02}", sign, h, m)
}

pub fn get_image_offset(file: &Path) -> Result<(String, bool)> {
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
    let output = run_capture("exiftool", args)?;

    let mut dst_on = false;
    let mut offset_time_orig: Option<String> = None;
    let mut timezone_val: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key_part, value_part)) = line.split_once(':') {
            let key = key_part.trim();
            let value = value_part.trim();

            let tag = if let Some((_, t)) = key.rsplit_once(' ') {
                t
            } else {
                key
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
        return Ok((oto, dst_on));
    }

    if let Some(tz) = timezone_val {
        let mut mins = parse_offset(&tz)?;
        if dst_on {
            mins += 60;
        }
        return Ok((format_offset(mins), dst_on));
    }

    Err(anyhow::anyhow!("No offset found in {:?}", file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_offset() {
        assert_eq!(parse_offset("+01:00").unwrap(), 60);
        assert_eq!(parse_offset("-05:00").unwrap(), -300);
        assert_eq!(parse_offset("+00:00").unwrap(), 0);
        assert_eq!(parse_offset("+05:30").unwrap(), 330);
    }

    #[test]
    fn test_format_offset() {
        assert_eq!(format_offset(60), "+01:00");
        assert_eq!(format_offset(-300), "-05:00");
        assert_eq!(format_offset(0), "+00:00");
        assert_eq!(format_offset(330), "+05:30");
    }
}
