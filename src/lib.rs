#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

pub mod cfg;
pub mod channel;
pub mod panel;

use self::channel::Channel;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{
    fs,
    io::{self, Read},
    path::Path,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct XfceConfig<'a> {
    pub channels: Vec<Channel<'a>>,
    pub panel_plugin_configs: Vec<self::panel::PluginConfig<'a>>,
}

impl XfceConfig<'static> {
    pub fn from_json_reader<R>(reader: R) -> Result<Self>
    where
        R: Read,
    {
        serde_json::from_reader(reader).map_err(Into::into)
    }

    pub fn from_env(xfce4_config_dir: &Path) -> Result<Self> {
        let channels_dir = xfce4_config_dir.join("xfconf/xfce-perchannel-xml");
        let panel_plugins_dir = xfce4_config_dir.join("panel");

        let channels = channels_dir
            .read_dir()
            .context("error reading channels dir")?
            .map(|entry| {
                let entry = entry.context("error reading dir entry")?;
                let path = entry.path();
                let file = fs::File::open(path)
                    .context("error opening channel XML file")?;
                let reader = io::BufReader::new(file);
                let channel = Channel::read_xml(reader)
                    .context("error reading channel XML")?;
                Ok(channel)
            })
            .collect::<Result<Vec<_>>>()
            .context("error loading channels data")?;

        let panel_plugin_configs = panel_plugins_dir
            .read_dir()
            .context("error reading panel plugins dir")?
            .map(|entry| {
                let entry = entry.context("error reading dir entry")?;
                let path = entry.path();
                self::panel::PluginConfig::from_path(&path)
            })
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>>>()
            .context("error loading panel plugins data")?;

        Ok(Self {
            channels,
            panel_plugin_configs,
        })
    }
}
