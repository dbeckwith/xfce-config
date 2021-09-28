#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

use anyhow::{Context, Result};
use xfce_config::XfceConfig;

fn main() -> Result<()> {
    let new_config = XfceConfig::from_json_reader(std::io::stdin())
        .context("error reading input JSON")?;
    dbg!(&new_config);

    let xfce4_config_dir = dirs2::config_dir()
        .context("could not get config dir")?
        .join("xfce4");

    let existing_config = XfceConfig::from_env(&xfce4_config_dir)
        .context("error reading config from environment")?;
    dbg!(&existing_config);

    Ok(())
}
