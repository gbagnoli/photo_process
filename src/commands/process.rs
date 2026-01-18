use crate::commands::download_gpx::cmd_download_gpx;
use crate::commands::geotag::cmd_geotag;
use crate::commands::organize::cmd_organize;
use crate::commands::rename::cmd_rename;
use crate::commands::set_time::cmd_set_time;
use crate::utils::{cmd_shift, detect_timezones, get_files_recursively, run_capture, AppConfig};
use anyhow::Result;
use chrono::NaiveDate;
use colored::Colorize;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn cmd_process(
    config: &AppConfig,
    dirs: &[PathBuf],
    timezone: &str,
    timezone_id: i32,
    dst: bool,
    organize: bool,
) -> Result<()> {
    // 1. Scan and Detect Timezones
    println!(
        "{}",
        "Scanning input directories for images and GPX files...".bold()
    );
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

        println!("Shifting to UTC by {}", shift_val);
        cmd_shift(config, false, &shift_val, &res.images)?;
    }

    // 3. Organize & Download GPX
    let mut min_date: Option<NaiveDate> = None;
    let mut max_date: Option<NaiveDate> = None;

    if organize {
        println!("{}", "Organizing photos...".bold());
        cmd_organize(config, dirs)?;

        // Determine date range from images metadata (works in dry-run too)
        for dir in dirs {
            let args = ["-T", "-d", "%Y-%m-%d", "-DateTimeOriginal", "-r"];
            let dir_str = dir.to_string_lossy().to_string();
            let output = run_capture(
                "exiftool",
                &[args[0], args[1], args[2], args[3], args[4], &dir_str],
            )?;

            for line in output.lines() {
                let date_str = line.trim();
                if date_str == "-" || date_str.is_empty() {
                    continue;
                }
                if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    if min_date.is_none() || date < min_date.unwrap() {
                        min_date = Some(date);
                    }
                    if max_date.is_none() || date > max_date.unwrap() {
                        max_date = Some(date);
                    }
                }
            }
        }

        if let (Some(start), Some(end)) = (min_date, max_date) {
            let start_str = start.format("%Y-%m-%d").to_string();
            let end_str = end.format("%Y-%m-%d").to_string();
            println!(
                "{}",
                format!("Detected date range: {} to {}", start_str, end_str).bold()
            );

            for dir in dirs {
                println!("{}", format!("Downloading GPX files to {:?}", dir).bold());
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
    } else if config.dry_run && organize && min_date.is_some() {
        println!(
            "{}",
            "DRY-RUN: Would geotag images using downloaded GPX files.".green()
        );
    } else {
        println!("No GPX files found, skipping geotag.");
    }

    // 7. Set Time (UTC -> Target)
    println!(
        "{}",
        format!("Setting time and timezone to {}", timezone).bold()
    );
    cmd_set_time(config, &all_images, true, timezone, timezone_id, dst)?;

    // 8. Rename
    cmd_rename(config, &all_images)?;

    Ok(())
}
