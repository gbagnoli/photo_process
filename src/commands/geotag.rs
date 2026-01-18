use crate::utils::{
    clean, ensure_gpx, get_all_images_from_paths, merge_gpx, resolve_files, run, AppConfig,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn cmd_geotag(config: &AppConfig, gps_files: &[PathBuf], paths: &[PathBuf]) -> Result<()> {
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
        println!("  -> Processing directory: {:?}", dir);

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
