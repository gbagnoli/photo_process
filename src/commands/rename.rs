use crate::utils::{clean, fix_extensions, get_all_images_from_paths, run, AppConfig};
use anyhow::Result;
use std::path::PathBuf;

pub fn cmd_rename(config: &AppConfig, paths: &[PathBuf]) -> Result<()> {
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
