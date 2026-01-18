use crate::utils::{get_files_recursively, remove_empty_dirs_recursive, run, AppConfig};
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

pub fn cmd_organize(config: &AppConfig, dirs: &[PathBuf]) -> Result<()> {
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
