#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

pub mod cfg;
pub mod channel;
pub mod panel;
mod serde;

use ::serde::Deserialize;
use anyhow::{Context, Result};
use std::{io::Read, path::Path};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct XfceConfig<'a> {
    pub channels: channel::Channels<'a>,
    pub panel_plugin_configs: panel::PluginConfigs<'a>,
}

#[derive(Debug)]
pub struct XfceConfigPatch<'a> {
    channels: channel::ChannelsPatch<'a>,
    panel_plugin_configs: panel::PluginConfigsPatch<'a>,
}

impl<'a> XfceConfigPatch<'a> {
    pub fn diff(old: &XfceConfig<'a>, new: &XfceConfig<'a>) -> Self {
        XfceConfigPatch {
            channels: channel::ChannelsPatch::diff(
                &old.channels,
                &new.channels,
            ),
            panel_plugin_configs: panel::PluginConfigsPatch::diff(
                &old.panel_plugin_configs,
                &new.panel_plugin_configs,
            ),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }
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
        let channels = channel::Channels::read(&channels_dir)
            .context("error loading channels data")?;
        let panel_plugin_configs =
            panel::PluginConfigs::read(&panel_plugins_dir)
                .context("error loading panel plugins data")?;
        Ok(Self {
            channels,
            panel_plugin_configs,
        })
    }
}

impl XfceConfigPatch<'_> {
    pub fn apply(self, dry_run: bool) -> Result<()> {
        self.channels
            .apply(
                &mut channel::Applier::new(dry_run)
                    .context("error creating channels applier")?,
            )
            .context("error applying channels")?;
        Ok(())
    }
}
