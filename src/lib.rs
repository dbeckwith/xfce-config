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
    pub id: u64,
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
    pub id: u64,
    pub r#type: Cow<'a, str>,
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

        let config_files = panel_plugins_dir
            .read_dir()
            .context("error reading panel plugins dir")?
            .map(|entry| {
                let entry = entry.context("error reading dir entry")?;
                let metadata = entry.metadata().context(
                    "error getting metadata for panel plugin dir entry",
                )?;
                let path = entry.path();

                let plugin = (|| {
                    let file_name = path.file_stem()?;
                    let file_name = file_name.to_str()?;
                    let (r#type, id) = file_name.rsplit_once('-')?;
                    let id = id.parse().ok()?;
                    let r#type = r#type.to_owned().into();
                    Some(ConfigFilePlugin { id, r#type })
                })();
                let plugin = if let Some(plugin) = plugin {
                    plugin
                } else {
                    return Ok(None);
                };

                let file = if metadata.is_dir() {
                    let files = path
                        .read_dir()
                        .context("error reading desktop dir")?
                        .map(|entry| {
                            let entry =
                                entry.context("error reading dir entry")?;
                            let metadata = entry.metadata().context(
                                "error getting metadata for desktop dir entry",
                            )?;
                            let path = entry.path();

                            let id = (|| {
                                let file_name = entry.file_name();
                                let file_name = file_name.to_str()?;
                                let (id, ext) = file_name.split_once('.')?;
                                if ext != "desktop" {
                                    return None;
                                }
                                let id = id.parse().ok()?;
                                Some(id)
                            })();
                            let id = if let Some(id) = id {
                                id
                            } else {
                                return Ok(None);
                            };

                            let content = if metadata.file_type().is_symlink() {
                                let path = path
                                    .read_link()
                                    .context("error reading desktop link")?;
                                DesktopFileContent::Link(DesktopFileLink {
                                    path: path.into(),
                                })
                            } else {
                                let file = fs::File::open(path)
                                    .context("error opening desktop file")?;
                                let reader = io::BufReader::new(file);
                                let cfg = Cfg::read(reader)
                                    .context("error reading desktop file")?;
                                DesktopFileContent::Cfg(cfg)
                            };

                            Ok(Some(DesktopFile { id, content }))
                        })
                        .filter_map(Result::transpose)
                        .collect::<Result<Vec<_>>>()
                        .context("error loading desktop files")?;
                    ConfigFileFile::DesktopDir(DesktopDir { files })
                } else {
                    let file = fs::File::open(path)
                        .context("error opening plugin rc file")?;
                    let reader = io::BufReader::new(file);
                    let cfg =
                        Cfg::read(reader).context("error reading plugin rc")?;
                    ConfigFileFile::Rc(cfg)
                };

                Ok(Some(ConfigFile { file, plugin }))
            })
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>>>()
            .context("error loading panel plugins data")?;

        Ok(Self {
            channels,
            config_files,
        })
    }
}
