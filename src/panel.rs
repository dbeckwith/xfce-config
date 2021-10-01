use crate::cfg::Cfg;
use anyhow::{Context, Result};
use serde::{de, Deserialize};
use std::{borrow::Cow, collections::BTreeMap, fs, io, path::Path};

#[derive(Debug, Deserialize)]
pub struct PluginConfig<'a> {
    pub plugin: PluginId<'a>,
    pub file: PluginConfigFile<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct PluginId<'a> {
    pub r#type: Cow<'a, str>,
    pub id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PluginConfigFile<'a> {
    Rc(Cfg<'a>),
    DesktopDir(DesktopDir<'a>),
}

#[derive(Debug, Deserialize)]
pub struct DesktopDir<'a> {
    #[serde(deserialize_with = "de_desktop_dir_files")]
    pub files: BTreeMap<u64, DesktopFile<'a>>,
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
    Link(Link<'a>),
}

#[derive(Debug, Deserialize)]
pub struct Link<'a> {
    pub path: Cow<'a, Path>,
}

impl PluginConfig<'static> {
    pub fn from_path(path: &Path) -> Result<Option<Self>> {
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

fn de_desktop_dir_files<'a, 'de, D>(
    deserializer: D,
) -> Result<BTreeMap<u64, DesktopFile<'a>>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let files = Vec::<DesktopFile<'_>>::deserialize(deserializer)?;
    Ok(files
        .into_iter()
        .map(|file| (file.id, file))
        .collect::<BTreeMap<_, _>>())
}
