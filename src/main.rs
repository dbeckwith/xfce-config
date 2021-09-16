#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

// TODO: make sure to read the existing config and merge it with the new one
// this way stateful or unknown props are preserved

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let config = serde_json::from_reader(std::io::stdin())
        .context("error reading input JSON")?;
    let (channel, plugin_configs) = xfce_config::convert(config);
    for xfce_config::PluginConfig {
        name: plugin_name,
        id: plugin_id,
        file,
    } in plugin_configs
    {
        match file {
            xfce_config::ConfigFile::Rc(cfg) => {
                eprintln!("{}-{}.rc:", plugin_name, plugin_id);
                eprintln!("========================================");
                cfg.write(std::io::stderr())
                    .context("error writing rc file")?;
                eprintln!("========================================");
            },
            xfce_config::ConfigFile::DesktopDir(
                xfce_config::ConfigFileDesktopDir { files },
            ) => {
                for (id, file) in files {
                    eprintln!("{}-{}/{}.desktop:", plugin_name, plugin_id, id);
                    match file {
                        xfce_config::ConfigDesktopFile::Cfg(cfg) => {
                            eprintln!(
                                "========================================"
                            );
                            cfg.write(std::io::stderr())
                                .context("error writing desktop file")?;
                            eprintln!(
                                "========================================"
                            );
                        },
                        xfce_config::ConfigDesktopFile::Link(path) => {
                            eprintln!("link from {}", path.display());
                        },
                    }
                }
            },
        }
    }
    channel
        .write_xml(std::io::stdout())
        .context("error writing channel XML")?;
    Ok(())
}
