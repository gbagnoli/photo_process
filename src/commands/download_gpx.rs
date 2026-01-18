use crate::utils::{ensure_gpx, get_files_recursively, merge_gpx, run, run_capture, AppConfig};
use anyhow::Result;
use chrono::{Duration, Local, NaiveDate};
use colored::Colorize;
use std::path::Path;

pub fn cmd_download_gpx(
    config: &AppConfig,
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

    println!(
        "{}",
        format!("Downloading activities from {} to {}", start, end).bold()
    );

    // Check if logged in
    let auth_status = run_capture("garmin", &["auth", "status"])?;
    if !auth_status.contains("Status: Logged in") {
        let msg = "You are not logged in to Garmin. Please run 'garmin auth login' first.";
        if config.dry_run {
            println!("{}", msg.truecolor(255, 100, 100)); // Light red
        } else {
            return Err(anyhow::anyhow!(msg));
        }
    }

    if !dest.exists() {
        std::fs::create_dir_all(dest)?;
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
                    println!(
                        "Activity {} already downloaded, checking name...{}",
                        activity_id,
                        if config.dry_run {
                            " (DRY-RUN)".green()
                        } else {
                            "".clear()
                        }
                    );
                    let _ = ensure_gpx(&gpx_path, config.dry_run)?;
                    continue;
                }

                println!(
                    "Downloading activity {} ({})...{}",
                    activity_id,
                    activity_date_str,
                    if config.dry_run {
                        " (DRY-RUN)".green()
                    } else {
                        "".clear()
                    }
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
                    config.dry_run,
                )?;

                // Rename the downloaded GPX file using its track name and time
                let _ = ensure_gpx(&gpx_path, config.dry_run)?;
            }
        }

        if !found_any || all_older {
            break;
        }
        offset += limit;
    }

    // After all downloads and renames, merge everything into all_activities.gpx
    let (_, gpx_files) = get_files_recursively(dest, config);
    if !gpx_files.is_empty() {
        let _ = merge_gpx(&gpx_files, dest, config.dry_run)?;
    }

    Ok(())
}
