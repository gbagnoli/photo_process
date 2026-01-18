use crate::utils::{detect_timezones, AppConfig};
use anyhow::Result;
use std::path::PathBuf;

pub fn cmd_detect_timezone(config: &AppConfig, paths: &[PathBuf]) -> Result<()> {
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
