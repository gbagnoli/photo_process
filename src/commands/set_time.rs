use crate::utils::{get_all_images_from_paths, resolve_files, run, AppConfig};
use anyhow::Result;
use std::path::PathBuf;

pub fn cmd_set_time(
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
