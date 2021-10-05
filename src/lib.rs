#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

mod cfg;
mod channel;
mod panel;
mod serde;

use ::serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct XfceConfig<'a> {
    channels: channel::Channels<'a>,
    panel_plugin_configs: panel::PluginConfigs<'a>,
}

#[derive(Debug, Serialize)]
pub struct XfceConfigPatch<'a> {
    #[serde(skip_serializing_if = "channel::ChannelsPatch::is_empty")]
    channels: channel::ChannelsPatch<'a>,
    #[serde(skip_serializing_if = "panel::PluginConfigsPatch::is_empty")]
    panel_plugin_configs: panel::PluginConfigsPatch<'a>,
}

impl<'a> XfceConfigPatch<'a> {
    pub fn diff(old: XfceConfig<'a>, new: XfceConfig<'a>) -> Self {
        XfceConfigPatch {
            channels: channel::ChannelsPatch::diff(old.channels, new.channels),
            panel_plugin_configs: panel::PluginConfigsPatch::diff(
                old.panel_plugin_configs,
                new.panel_plugin_configs,
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
        let channels = channel::Channels::read(
            &xfce4_config_dir.join("xfconf/xfce-perchannel-xml"),
        )
        .context("error loading channels data")?;
        let panel_plugin_configs =
            panel::PluginConfigs::read(&xfce4_config_dir.join("panel"))
                .context("error loading panel plugins data")?;
        Ok(Self {
            channels,
            panel_plugin_configs,
        })
    }
}

pub struct Applier {
    dry_run: bool,
    patch_recorder: PatchRecorder,
    xfce4_config_dir: PathBuf,
}

struct PatchRecorder {
    file: fs::File,
}

impl Applier {
    pub fn new(
        dry_run: bool,
        log_dir: &Path,
        xfce4_config_dir: PathBuf,
    ) -> Result<Self> {
        let patch_recorder = PatchRecorder::new(&log_dir.join("patches.json"))
            .context("error creating patch recorder")?;
        Ok(Self {
            dry_run,
            patch_recorder,
            xfce4_config_dir,
        })
    }
}

impl XfceConfigPatch<'_> {
    pub fn apply(self, applier: &mut Applier) -> Result<()> {
        self.channels
            .apply(
                &mut channel::Applier::new(
                    applier.dry_run,
                    &mut applier.patch_recorder,
                )
                .context("error creating channels applier")?,
            )
            .context("error applying channels")?;
        self.panel_plugin_configs
            .apply(&mut panel::Applier::new(
                applier.dry_run,
                &mut applier.patch_recorder,
                applier.xfce4_config_dir.join("panel"),
            ))
            .context("error applying panel plugin configs")?;
        Ok(())
    }
}

impl PatchRecorder {
    fn new(path: &Path) -> Result<Self> {
        let file = fs::File::create(path)?;
        Ok(Self { file })
    }

    fn log(&mut self, event: &PatchEvent<'_>) -> Result<()> {
        serde_json::to_writer(&mut self.file, event)?;
        writeln!(&mut self.file)?;
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
enum PatchEvent<'a> {
    Channel(channel::PatchEvent<'a>),
    Panel(panel::PatchEvent<'a>),
}
