#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

pub mod cfg;
pub mod channel;

use self::{cfg::*, channel::*};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{
    borrow::Cow,
    fs,
    io::{self, Read},
    path::Path,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct XfceConfig<'a> {
    pub channels: Vec<Channel<'a>>,
    pub config_files: Vec<ConfigFile<'a>>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFile<'a> {
    pub file: ConfigFileFile<'a>,
    pub plugin: ConfigFilePlugin<'a>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ConfigFileFile<'a> {
    Rc(Cfg<'a>),
    DesktopDir(DesktopDir<'a>),
}

#[derive(Debug, Deserialize)]
pub struct DesktopDir<'a> {
    pub files: Vec<DesktopFile<'a>>,
}

#[derive(Debug, Deserialize)]
pub struct DesktopFile<'a> {
    pub id: u32,
    pub content: DesktopFileContent<'a>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DesktopFileContent<'a> {
    Cfg(Cfg<'a>),
    Link(DesktopFileLink<'a>),
}

#[derive(Debug, Deserialize)]
pub struct DesktopFileLink<'a> {
    pub path: Cow<'a, Path>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFilePlugin<'a> {
    pub id: u32,
    pub r#type: Cow<'a, str>,
}

impl XfceConfig<'static> {
    pub fn from_json_reader<R>(reader: R) -> Result<Self>
    where
        R: Read,
    {
        serde_json::from_reader(reader).map_err(Into::into)
    }

    pub fn from_env() -> Result<Self> {
        let config_dir =
            dirs2::config_dir().context("could not get config dir")?;
        let channels_dir = config_dir.join("xfce4/xfconf/xfce-perchannel-xml");
        let panel_plugins_dir = config_dir.join("xfce4/panel");

        let channels = channels_dir
            .read_dir()
            .context("error listing channels dir")?
            .map(|entry| {
                let entry = entry.context("error reading dir entry")?;
                let path = entry.path();
                let file =
                    fs::File::open(path).context("error opening file")?;
                let reader = io::BufReader::new(file);
                let channel = Channel::read_xml(reader)
                    .context("error reading channel XML")?;
                Ok(channel)
            })
            .collect::<Result<Vec<_>>>()
            .context("error loading channels data")?;

        // TODO: read config files
        let config_files = Vec::new();

        Ok(Self {
            channels,
            config_files,
        })
    }
}
