#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

use anyhow::{Context, Result};
use std::fs;
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

    // TODO: unique log subdir for each run?
    let log_dir = dirs2::data_local_dir()
        .context("could not get data local dir")?
        .join("xfce-config");
    fs::create_dir_all(&log_dir).context("error creating log dir")?;

    let xfce4_config_dir = dirs2::config_dir()
        .context("could not get config dir")?
        .join("xfce4");

    let new_config = XfceConfig::from_json_reader(std::io::stdin())
        .context("error reading input JSON")?;
    serde_json::to_writer(
        fs::File::create(log_dir.join("new.json"))
            .context("error creating new.json")?,
        &new_config,
    )
    .context("error writing new.json")?;

    let old_config = XfceConfig::from_env(&xfce4_config_dir)
        .context("error reading config from environment")?;
    serde_json::to_writer(
        fs::File::create(log_dir.join("old.json"))
            .context("error creating old.json")?,
        &old_config,
    )
    .context("error writing old.json")?;

    let diff = XfceConfigPatch::diff(old_config, new_config);
    dbg!(&diff);

    diff.apply(&mut Applier::new(dry_run, xfce4_config_dir))
        .context("error applying config")?;

    Ok(())
}
