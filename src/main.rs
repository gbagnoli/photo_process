mod cli;
mod commands;
mod utils;

use crate::cli::{Cli, Commands};
use crate::commands::detect_timezone::cmd_detect_timezone;
use crate::commands::download_gpx::cmd_download_gpx;
use crate::commands::geotag::cmd_geotag;
use crate::commands::organize::cmd_organize;
use crate::commands::process::cmd_process;
use crate::commands::rename::cmd_rename;
use crate::commands::set_time::cmd_set_time;
use crate::commands::shift::cmd_shift;
use crate::commands::shift_to_utc::cmd_shift_to_utc;
use crate::utils::{get_tz_info, AppConfig};
use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let dry_run = match &cli.command {
        Commands::Process { force, .. } => !*force,
        _ => false,
    };

    let config = AppConfig {
        suffixes: cli.suffix.iter().map(|s| s.to_lowercase()).collect(),
        timerange: cli.timerange,
        dry_run,
    };

    match &cli.command {
        Commands::Rename { paths } => cmd_rename(&config, paths)?,
        Commands::SetTime {
            paths,
            timezone,
            dst,
        } => {
            let (tz_id, tz_info) = get_tz_info(timezone)?;
            cmd_set_time(&config, paths, false, &tz_info, tz_id, *dst)?
        }
        Commands::Geotag { gps_files, paths } => cmd_geotag(&config, gps_files, paths)?,
        Commands::Shift {
            reset_tz,
            by,
            paths,
        } => cmd_shift(&config, *reset_tz, by, paths)?,
        Commands::ShiftToUtc { paths } => cmd_shift_to_utc(&config, paths)?,
        Commands::DetectTimezone { paths } => cmd_detect_timezone(&config, paths)?,
        Commands::Organize { dirs } => cmd_organize(&config, dirs)?,
        Commands::Process {
            dirs,
            timezone,
            dst,
            organize,
            ..
        } => {
            let (tz_id, tz_info) = get_tz_info(timezone)?;
            cmd_process(&config, dirs, &tz_info, tz_id, *dst, *organize)?
        }
        Commands::DownloadGpx {
            dest,
            start_date,
            end_date,
        } => {
            cmd_download_gpx(&config, dest, start_date.as_ref(), end_date.as_ref())?;
        }
    }

    Ok(())
}
