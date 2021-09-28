#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

use anyhow::{Context, Result};
use xfce_config::XfceConfig;

fn main() -> Result<()> {
    let new_config = XfceConfig::from_json_reader(std::io::stdin())
        .context("error reading input JSON")?;
    dbg!(&new_config);

    let existing_config = XfceConfig::from_env()
        .context("error reading config from environment")?;
    dbg!(&existing_config);

    Ok(())
}
