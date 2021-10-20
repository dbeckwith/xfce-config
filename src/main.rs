#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

use anyhow::{Context, Result};
use std::{fs, path::PathBuf};
use structopt::StructOpt;
use xfce_config::{Applier, XfceConfig, XfceConfigPatch};

#[derive(StructOpt)]
struct Args {
    #[structopt(long)]
    apply: bool,
}

fn main() -> Result<()> {
    let args = Args::from_args();

    let dry_run = !args.apply;

    let log_dir = rotating_log_dir(
        dirs2::data_local_dir()
            .context("could not get data local dir")?
            .join("xfce-config"),
    )?;

    let config_dir = dirs2::config_dir().context("could not get config dir")?;
    let xfce4_config_dir = config_dir.join("xfce4");
    let gtk_config_dir = config_dir.join("gtk-3.0");

    let new_config = XfceConfig::from_json_reader(std::io::stdin())
        .context("error reading input JSON")?;
    serde_json::to_writer(
        fs::File::create(log_dir.join("new.json"))
            .context("error creating new.json")?,
        &new_config,
    )
    .context("error writing new.json")?;

    let old_config = XfceConfig::from_env(&xfce4_config_dir, &gtk_config_dir)
        .context("error reading config from environment")?;
    serde_json::to_writer(
        fs::File::create(log_dir.join("old.json"))
            .context("error creating old.json")?,
        &old_config,
    )
    .context("error writing old.json")?;

    let diff = XfceConfigPatch::diff(old_config, new_config);
    serde_json::to_writer(
        fs::File::create(log_dir.join("diff.json"))
            .context("error creating diff.json")?,
        &diff,
    )
    .context("error writing diff.json")?;

    diff.apply(
        &mut Applier::new(dry_run, &log_dir, xfce4_config_dir, gtk_config_dir)
            .context("error creating applier")?,
    )
    .context("error applying config")?;

    Ok(())
}

fn rotating_log_dir(dir: PathBuf) -> Result<PathBuf> {
    let log_dir =
        dir.join(chrono::Local::now().format("%Y-%m-%d-%H-%M-%S").to_string());
    fs::create_dir_all(&log_dir).context("error creating log dir")?;
    let mut log_dirs = fs::read_dir(dir)
        .context("error reading log dir")?
        .filter_map(|entry| {
            entry
                .context("error reading log dir entry")
                .map(|entry| {
                    let path = entry.path();
                    path.is_dir().then(|| path)
                })
                .transpose()
        })
        .collect::<Result<Vec<_>>>()?;
    log_dirs.sort();
    // remove all but the last 10
    for expired_log_dir in log_dirs.into_iter().rev().skip(10) {
        fs::remove_dir_all(expired_log_dir)
            .context("error removing old log dir")?;
    }
    Ok(log_dir)
}
