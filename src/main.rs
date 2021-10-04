#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

use anyhow::{Context, Result};
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
    dbg!(dry_run);

    let new_config = XfceConfig::from_json_reader(std::io::stdin())
        .context("error reading input JSON")?;
    dbg!(&new_config);

    let xfce4_config_dir = dirs2::config_dir()
        .context("could not get config dir")?
        .join("xfce4");

    let existing_config = XfceConfig::from_env(&xfce4_config_dir)
        .context("error reading config from environment")?;
    dbg!(&existing_config);

    let diff = XfceConfigPatch::diff(existing_config, new_config);
    dbg!(&diff);

    diff.apply(&mut Applier::new(dry_run, xfce4_config_dir))
        .context("error applying config")?;

    Ok(())
}
