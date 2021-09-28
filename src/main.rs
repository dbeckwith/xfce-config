#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let config = serde_json::from_reader(std::io::stdin())
        .context("error reading input JSON")?;
    dbg!(config);
    Ok(())
}
