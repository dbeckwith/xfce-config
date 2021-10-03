use crate::{
    cfg::{Cfg, CfgPatch},
    serde::IdMap,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs,
    io,
    path::Path,
};

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

trait Patch {
    type Data;

    fn diff(old: Self::Data, new: Self::Data) -> Self;

    fn is_empty(&self) -> bool;
}

#[derive(Debug)]
struct MapPatch<K, V>
where
    K: Ord,
    V: Patch,
{
    changed: BTreeMap<K, V>,
    added: BTreeMap<K, V::Data>,
    removed: BTreeSet<K>,
}

impl<K, V> Patch for MapPatch<K, V>
where
    K: Clone + Ord,
    V: Patch,
{
    type Data = BTreeMap<K, V::Data>;

    fn diff(mut old: Self::Data, new: Self::Data) -> Self {
        let mut changed = BTreeMap::new();
        let mut added = BTreeMap::new();
        for (key, new_value) in new.into_iter() {
            if let Some(old_value) = old.remove(&key) {
                let patch = V::diff(old_value, new_value);
                if !patch.is_empty() {
                    changed.insert(key, patch);
                }
            } else {
                added.insert(key, new_value);
            }
        }
        let removed = old.into_keys().collect::<BTreeSet<_>>();
        Self {
            changed,
            added,
            removed,
        }
    }

    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty()
    }
}

#[derive(Debug)]
pub struct PluginConfigsPatch<'a>(
    MapPatch<PluginId<'a>, PluginConfigPatch<'a>>,
);

impl<'a> PluginConfigsPatch<'a> {
    pub fn diff(old: PluginConfigs<'a>, new: PluginConfigs<'a>) -> Self {
        Self(MapPatch::diff((old.0).0, (new.0).0))
    }
}

#[derive(Debug)]
enum PluginConfigPatch<'a> {
    Rc(PluginId<'a>, CfgPatch<'a>),
    DesktopDir(PluginId<'a>, DesktopDirPatch<'a>),
    Changed(PluginConfig<'a>),
}

impl<'a> Patch for PluginConfigPatch<'a> {
    type Data = PluginConfig<'a>;

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        match (old, new) {
            (
                PluginConfig {
                    plugin: _,
                    file: PluginConfigFile::Rc(old_rc),
                },
                PluginConfig {
                    plugin,
                    file: PluginConfigFile::Rc(new_rc),
                },
            ) => Self::Rc(plugin, CfgPatch::diff(old_rc, new_rc)),
            (
                PluginConfig {
                    plugin: _,
                    file: PluginConfigFile::DesktopDir(old_desktop_dir),
                },
                PluginConfig {
                    plugin,
                    file: PluginConfigFile::DesktopDir(new_desktop_dir),
                },
            ) => Self::DesktopDir(
                plugin,
                DesktopDirPatch::diff(old_desktop_dir, new_desktop_dir),
            ),
            (_old, new) => Self::Changed(new),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            PluginConfigPatch::Rc(_, cfg_patch) => cfg_patch.is_empty(),
            PluginConfigPatch::DesktopDir(_, desktop_dir_patch) => {
                desktop_dir_patch.is_empty()
            },
            PluginConfigPatch::Changed(_) => false,
        }
    }
}

#[derive(Debug)]
struct DesktopDirPatch<'a>(MapPatch<u64, DesktopFilePatch<'a>>);

impl<'a> Patch for DesktopDirPatch<'a> {
    type Data = DesktopDir<'a>;

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        Self(MapPatch::diff(old.files.0, new.files.0))
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug)]
enum DesktopFilePatch<'a> {
    Cfg(u64, CfgPatch<'a>),
    Link(u64, LinkPatch<'a>),
    Changed(DesktopFile<'a>),
}

impl<'a> Patch for DesktopFilePatch<'a> {
    type Data = DesktopFile<'a>;

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        match (old, new) {
            (
                DesktopFile {
                    id: _,
                    content: DesktopFileContent::Cfg(old_cfg),
                },
                DesktopFile {
                    id,
                    content: DesktopFileContent::Cfg(new_cfg),
                },
            ) => Self::Cfg(id, CfgPatch::diff(old_cfg, new_cfg)),
            (
                DesktopFile {
                    id: _,
                    content: DesktopFileContent::Link(old_link),
                },
                DesktopFile {
                    id,
                    content: DesktopFileContent::Link(new_link),
                },
            ) => Self::Link(id, LinkPatch::diff(old_link, new_link)),
            (_old, new) => Self::Changed(new),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            DesktopFilePatch::Cfg(_, cfg_patch) => cfg_patch.is_empty(),
            DesktopFilePatch::Link(_, link_patch) => link_patch.is_empty(),
            DesktopFilePatch::Changed(_) => false,
        }
    }
}

#[derive(Debug)]
struct LinkPatch<'a> {
    value: Option<Cow<'a, Path>>,
}

impl<'a> Patch for LinkPatch<'a> {
    type Data = Link<'a>;

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        Self {
            value: (old.path != new.path).then(|| new.path),
        }
    }

    fn is_empty(&self) -> bool {
        self.value.is_none()
    }
}
