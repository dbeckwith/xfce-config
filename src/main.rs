#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

// TODO: make sure to read the existing config and merge it with the new one
// this way stateful or unknown props are preserved

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let config = serde_json::from_reader(std::io::stdin())
        .context("error reading input JSON")?;
    let (channel, config_files) = xfce_config::convert(config);
    for config_file in config_files {
        match config_file {
            xfce_config::ConfigFile::Link(xfce_config::ConfigFileLink {
                from,
                to,
            }) => {
                eprintln!("{}:", from.display());
                eprintln!("link to {}", to.display());
            },
            xfce_config::ConfigFile::File(xfce_config::ConfigFileFile {
                path,
                contents,
            }) => {
                eprintln!("{}:", path.display());
                eprintln!("========================================");
                contents
                    .write(std::io::stderr())
                    .context("error writing cfg file")?;
                eprintln!("========================================");
            },
        }
    }
    channel
        .write_xml(std::io::stdout())
        .context("error writing channel XML")?;
    Ok(())
}
