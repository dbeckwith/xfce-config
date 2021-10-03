use crate::{cfg::Cfg, serde::IdMap};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{borrow::Cow, collections::BTreeMap, fs, io, path::Path};

#[derive(Debug, Deserialize)]
pub struct PluginConfigs<'a>(IdMap<PluginConfig<'a>>);

#[derive(Debug, Deserialize)]
struct PluginConfig<'a> {
    plugin: PluginId<'a>,
    file: PluginConfigFile<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
struct PluginId<'a> {
    r#type: Cow<'a, str>,
    id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum PluginConfigFile<'a> {
    Rc(Cfg<'a>),
    DesktopDir(DesktopDir<'a>),
}

#[derive(Debug, Deserialize)]
struct DesktopDir<'a> {
    files: IdMap<DesktopFile<'a>>,
}

#[derive(Debug, Deserialize)]
struct DesktopFile<'a> {
    id: u64,
    content: DesktopFileContent<'a>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum DesktopFileContent<'a> {
    Cfg(Cfg<'a>),
    Link(Link<'a>),
}

#[derive(Debug, Deserialize)]
struct Link<'a> {
    path: Cow<'a, Path>,
}

impl PluginConfigs<'static> {
    pub fn read(dir: &Path) -> Result<Self> {
        dir.read_dir()
            .context("error reading dir")?
            .map(|entry| {
                let entry = entry.context("error reading dir entry")?;
                let path = entry.path();
                PluginConfig::read(&path)
            })
            .filter_map(Result::transpose)
            .map(|plugin_config| {
                plugin_config.map(|plugin_config| {
                    (plugin_config.plugin.clone(), plugin_config)
                })
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(IdMap)
            .map(Self)
    }
}

impl PluginConfig<'static> {
    fn read(path: &Path) -> Result<Option<Self>> {
        let plugin = (|| {
            let file_name = path.file_stem()?;
            let file_name = file_name.to_str()?;
            let (r#type, id) = file_name.rsplit_once('-')?;
            let id = id.parse().ok()?;
            let r#type = r#type.to_owned().into();
            Some(PluginId { id, r#type })
        })();
        let plugin = if let Some(plugin) = plugin {
            plugin
        } else {
            return Ok(None);
        };

        let file = if path.is_dir() {
            let files = path
                .read_dir()
                .context("error reading desktop dir")?
                .map(|entry| {
                    let entry = entry.context("error reading dir entry")?;
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
                        DesktopFileContent::Link(Link { path: path.into() })
                    } else {
                        let file = fs::File::open(path)
                            .context("error opening desktop file")?;
                        let reader = io::BufReader::new(file);
                        let cfg = Cfg::read(reader)
                            .context("error reading desktop file")?;
                        DesktopFileContent::Cfg(cfg)
                    };

                    Ok(Some((id, DesktopFile { id, content })))
                })
                .filter_map(Result::transpose)
                .collect::<Result<BTreeMap<_, _>>>()
                .map(IdMap)
                .context("error loading desktop files")?;
            PluginConfigFile::DesktopDir(DesktopDir { files })
        } else {
            let file =
                fs::File::open(path).context("error opening plugin rc file")?;
            let reader = io::BufReader::new(file);
            let cfg = Cfg::read(reader).context("error reading plugin rc")?;
            PluginConfigFile::Rc(cfg)
        };

        Ok(Some(PluginConfig { file, plugin }))
    }
}

impl<'a> crate::serde::Id for PluginConfig<'a> {
    type Id = PluginId<'a>;

    fn id(&self) -> &Self::Id {
        &self.plugin
    }
}

impl crate::serde::Id for DesktopFile<'_> {
    type Id = u64;

    fn id(&self) -> &Self::Id {
        &self.id
    }
}
