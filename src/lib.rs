#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

pub mod cfg;
pub mod channel;
pub mod diff;
pub mod panel;

use anyhow::{Context, Result};
use channel::Channel;
use serde::{de, Deserialize};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::Path,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct XfceConfig<'a> {
    #[serde(deserialize_with = "de_xfce_config_channels")]
    pub channels: BTreeMap<Cow<'a, str>, Channel<'a>>,
    #[serde(deserialize_with = "de_xfce_config_panel_plugin_configs")]
    pub panel_plugin_configs:
        BTreeMap<panel::PluginId<'a>, panel::PluginConfig<'a>>,
}

#[derive(Debug)]
pub struct XfceConfigPatch<'a> {
    channels: <BTreeMap<Cow<'a, str>, Channel<'a>> as diff::Diff>::Patch,
}

impl<'a> diff::Diff for XfceConfig<'a> {
    type Patch = XfceConfigPatch<'a>;

    fn diff(&self, other: &Self) -> Self::Patch {
        XfceConfigPatch {
            channels: self.channels.diff(&other.channels),
        }
    }
}

impl diff::Patch for XfceConfigPatch<'_> {
    fn is_empty(&self) -> bool {
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
            .map(|channel| {
                channel.map(|channel| (channel.name.clone(), channel))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .context("error loading channels data")?;

        let panel_plugin_configs = panel_plugins_dir
            .read_dir()
            .context("error reading panel plugins dir")?
            .map(|entry| {
                let entry = entry.context("error reading dir entry")?;
                let path = entry.path();
                panel::PluginConfig::from_path(&path)
            })
            .filter_map(Result::transpose)
            .map(|plugin_config| {
                plugin_config.map(|plugin_config| {
                    (plugin_config.plugin.clone(), plugin_config)
                })
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .context("error loading panel plugins data")?;

        Ok(Self {
            channels,
            panel_plugin_configs,
        })
    }
}

fn de_xfce_config_channels<'a, 'de, D>(
    deserializer: D,
) -> Result<BTreeMap<Cow<'a, str>, Channel<'a>>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let channels = Vec::<Channel<'_>>::deserialize(deserializer)?;
    Ok(channels
        .into_iter()
        .map(|channel| (channel.name.clone(), channel))
        .collect::<BTreeMap<_, _>>())
}

fn de_xfce_config_panel_plugin_configs<'a, 'de, D>(
    deserializer: D,
) -> Result<BTreeMap<panel::PluginId<'a>, panel::PluginConfig<'a>>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let panel_plugin_configs =
        Vec::<panel::PluginConfig<'_>>::deserialize(deserializer)?;
    Ok(panel_plugin_configs
        .into_iter()
        .map(|plugin_config| (plugin_config.plugin.clone(), plugin_config))
        .collect::<BTreeMap<_, _>>())
}
