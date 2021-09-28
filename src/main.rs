#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

use anyhow::{Context, Result};
use xfce_config::XfceConfig;

fn main() -> Result<()> {
    let config: XfceConfig<'static> = serde_json::from_reader(std::io::stdin())
        .context("error reading input JSON")?;
    dbg!(config);
    Ok(())
}
