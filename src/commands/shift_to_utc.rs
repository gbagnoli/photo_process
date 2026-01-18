use crate::commands::shift::cmd_shift;
use crate::utils::{detect_timezones, AppConfig};
use anyhow::Result;
use std::path::PathBuf;

pub fn cmd_shift_to_utc(config: &AppConfig, paths: &[PathBuf]) -> Result<()> {
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
