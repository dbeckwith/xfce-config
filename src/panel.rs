use crate::{
    cfg::{Applier as CfgApplier, Cfg, CfgPatch},
    serde::IdMap,
    PatchRecorder,
};
use anyhow::{bail, Context, Result};
use cfg_if::cfg_if;
use serde::{ser, Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs,
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Panel<'a> {
    plugin_configs: PluginConfigs<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PluginConfigs<'a>(IdMap<PluginConfig<'a>>);

#[derive(Debug, Serialize, Deserialize)]
struct PluginConfig<'a> {
    #[serde(rename = "plugin")]
    id: PluginId<'a>,
    file: PluginConfigFile<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
struct PluginId<'a> {
    r#type: Cow<'a, str>,
    id: u64,
}

impl fmt::Display for PluginId<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.r#type, self.id)
    }
}

impl ser::Serialize for PluginId<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.collect_str(&format_args!("{}-{}", self.r#type, self.id))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum PluginConfigFile<'a> {
    Rc(Cfg<'a>),
    DesktopDir(DesktopDir<'a>),
}

#[derive(Debug, Serialize, Deserialize)]
struct DesktopDir<'a> {
    files: IdMap<DesktopFile<'a>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DesktopFile<'a> {
    id: u64,
    content: DesktopFileContent<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum DesktopFileContent<'a> {
    Cfg(Cfg<'a>),
    Link(Link<'a>),
}

#[derive(Debug, Serialize, Deserialize)]
struct Link<'a> {
    path: Cow<'a, Path>,
}

impl Panel<'static> {
    pub fn read(dir: &Path) -> Result<Self> {
        Ok(Self {
            plugin_configs: PluginConfigs::read(dir)
                .context("error reading plugin configs")?,
        })
    }
}

impl PluginConfigs<'static> {
    fn read(dir: &Path) -> Result<Self> {
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
                    (plugin_config.id.clone(), plugin_config)
                })
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(IdMap)
            .map(Self)
    }
}

impl PluginConfig<'static> {
    fn read(path: &Path) -> Result<Option<Self>> {
        let id = (|| {
            let file_name = path.file_stem()?;
            let file_name = file_name.to_str()?;
            let (r#type, id) = file_name.rsplit_once('-')?;
            let id = id.parse().ok()?;
            let r#type = r#type.to_owned().into();
            Some(PluginId { id, r#type })
        })();
        let id = if let Some(id) = id {
            id
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
        } else if path.extension().and_then(std::ffi::OsStr::to_str)
            == Some("rc")
        {
            let file =
                fs::File::open(path).context("error opening plugin RC file")?;
            let reader = io::BufReader::new(file);
            let cfg = Cfg::read(reader).context("error reading plugin RC")?;
            PluginConfigFile::Rc(cfg)
        } else {
            return Ok(None);
        };

        Ok(Some(PluginConfig { id, file }))
    }
}

impl<'a> crate::serde::Id for PluginConfig<'a> {
    type Id = PluginId<'a>;

    fn id(&self) -> &Self::Id {
        &self.id
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

#[derive(Debug, Serialize)]
#[serde(bound(
    serialize = "K: Ord + Serialize, V: Patch + Serialize, V::Data: Serialize"
))]
struct MapPatch<K, V>
where
    K: Ord,
    V: Patch,
{
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    changed: BTreeMap<K, V>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    added: BTreeMap<K, V::Data>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
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

#[derive(Debug, Serialize)]
pub struct PanelPatch<'a> {
    #[serde(skip_serializing_if = "PluginConfigsPatch::is_empty")]
    plugin_configs: PluginConfigsPatch<'a>,
}

impl<'a> PanelPatch<'a> {
    pub fn diff(old: Panel<'a>, new: Panel<'a>) -> Self {
        Self {
            plugin_configs: PluginConfigsPatch::diff(
                old.plugin_configs,
                new.plugin_configs,
            ),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.plugin_configs.is_empty()
    }
}

#[derive(Debug, Serialize)]
struct PluginConfigsPatch<'a>(MapPatch<PluginId<'a>, PluginConfigPatch<'a>>);

impl<'a> PluginConfigsPatch<'a> {
    fn diff(old: PluginConfigs<'a>, new: PluginConfigs<'a>) -> Self {
        Self(MapPatch::diff((old.0).0, (new.0).0))
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Serialize)]
enum PluginConfigPatch<'a> {
    Rc(RcPatch<'a>),
    DesktopDir(DesktopDirPatch<'a>),
    Changed(PluginConfig<'a>),
}

impl<'a> Patch for PluginConfigPatch<'a> {
    type Data = PluginConfig<'a>;

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        match (old, new) {
            (
                PluginConfig {
                    id: old_id,
                    file: PluginConfigFile::Rc(old_rc),
                },
                PluginConfig {
                    id: new_id,
                    file: PluginConfigFile::Rc(new_rc),
                },
            ) => Self::Rc(RcPatch::diff((old_id, old_rc), (new_id, new_rc))),
            (
                PluginConfig {
                    id: old_id,
                    file: PluginConfigFile::DesktopDir(old_desktop_dir),
                },
                PluginConfig {
                    id: new_id,
                    file: PluginConfigFile::DesktopDir(new_desktop_dir),
                },
            ) => Self::DesktopDir(DesktopDirPatch::diff(
                (old_id, old_desktop_dir),
                (new_id, new_desktop_dir),
            )),
            (_old, new) => Self::Changed(new),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Rc(rc_patch) => rc_patch.is_empty(),
            Self::DesktopDir(desktop_dir_patch) => desktop_dir_patch.is_empty(),
            Self::Changed(_) => false,
        }
    }
}

#[derive(Debug, Serialize)]
struct RcPatch<'a> {
    id: PluginId<'a>,
    cfg: CfgPatch<'a>,
}

impl<'a> Patch for RcPatch<'a> {
    type Data = (PluginId<'a>, Cfg<'a>);

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        Self {
            id: new.0,
            cfg: CfgPatch::diff(old.1, new.1),
        }
    }

    fn is_empty(&self) -> bool {
        self.cfg.is_empty()
    }
}

#[derive(Debug, Serialize)]
struct DesktopDirPatch<'a> {
    id: PluginId<'a>,
    files: MapPatch<u64, DesktopFilePatch<'a>>,
}

impl<'a> Patch for DesktopDirPatch<'a> {
    type Data = (PluginId<'a>, DesktopDir<'a>);

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        Self {
            id: new.0,
            files: MapPatch::diff(old.1.files.0, new.1.files.0),
        }
    }

    fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[derive(Debug, Serialize)]
enum DesktopFilePatch<'a> {
    Cfg(DesktopFileCfgPatch<'a>),
    Link(LinkPatch<'a>),
    Changed(DesktopFile<'a>),
}

impl<'a> Patch for DesktopFilePatch<'a> {
    type Data = DesktopFile<'a>;

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        match (old, new) {
            (
                DesktopFile {
                    id: old_id,
                    content: DesktopFileContent::Cfg(old_cfg),
                },
                DesktopFile {
                    id: new_id,
                    content: DesktopFileContent::Cfg(new_cfg),
                },
            ) => Self::Cfg(DesktopFileCfgPatch::diff(
                (old_id, old_cfg),
                (new_id, new_cfg),
            )),
            (
                DesktopFile {
                    id: old_id,
                    content: DesktopFileContent::Link(old_link),
                },
                DesktopFile {
                    id: new_id,
                    content: DesktopFileContent::Link(new_link),
                },
            ) => Self::Link(LinkPatch::diff(
                (old_id, old_link),
                (new_id, new_link),
            )),
            (_old, new) => Self::Changed(new),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Cfg(desktop_file_cfg_patch) => {
                desktop_file_cfg_patch.is_empty()
            },
            Self::Link(link_patch) => link_patch.is_empty(),
            Self::Changed(_) => false,
        }
    }
}

#[derive(Debug, Serialize)]
struct DesktopFileCfgPatch<'a> {
    id: u64,
    cfg: CfgPatch<'a>,
}

impl<'a> Patch for DesktopFileCfgPatch<'a> {
    type Data = (u64, Cfg<'a>);

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        Self {
            id: new.0,
            cfg: CfgPatch::diff(old.1, new.1),
        }
    }

    fn is_empty(&self) -> bool {
        self.cfg.is_empty()
    }
}

#[derive(Debug, Serialize)]
struct LinkPatch<'a> {
    id: u64,
    path: Option<Cow<'a, Path>>,
}

impl<'a> Patch for LinkPatch<'a> {
    type Data = (u64, Link<'a>);

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        Self {
            id: new.0,
            path: (old.1.path != new.1.path).then(|| new.1.path),
        }
    }

    fn is_empty(&self) -> bool {
        self.path.is_none()
    }
}

pub struct Applier<'a> {
    dry_run: bool,
    patch_recorder: &'a mut PatchRecorder,
    dir: PathBuf,
}

impl<'a> Applier<'a> {
    pub(crate) fn new(
        dry_run: bool,
        patch_recorder: &'a mut PatchRecorder,
        dir: PathBuf,
    ) -> Self {
        Self {
            dry_run,
            patch_recorder,
            dir,
        }
    }

    fn log(&mut self, event: PatchEvent<'_>) -> Result<()> {
        self.patch_recorder.log(&crate::PatchEvent::Panel(event))
    }

    fn rc_file_path(&self, plugin_id: &PluginId<'_>) -> PathBuf {
        self.dir
            .join(format!("{}-{}.rc", plugin_id.r#type, plugin_id.id))
    }

    fn desktop_dir_path(&self, plugin_id: &PluginId<'_>) -> PathBuf {
        self.dir
            .join(format!("{}-{}", plugin_id.r#type, plugin_id.id))
    }

    fn desktop_file_path(
        &self,
        plugin_id: &PluginId<'_>,
        desktop_id: u64,
    ) -> PathBuf {
        self.dir.join(format!(
            "{}-{}/{}.desktop",
            plugin_id.r#type, plugin_id.id, desktop_id
        ))
    }

    fn rc_cfg_applier(&mut self, plugin_id: &PluginId<'_>) -> CfgApplier<'_> {
        CfgApplier::new(
            self.dry_run,
            self.patch_recorder,
            self.rc_file_path(plugin_id),
        )
    }

    fn desktop_cfg_applier(
        &mut self,
        plugin_id: &PluginId<'_>,
        desktop_id: u64,
    ) -> CfgApplier<'_> {
        CfgApplier::new(
            self.dry_run,
            self.patch_recorder,
            self.desktop_file_path(plugin_id, desktop_id),
        )
    }

    fn remove_plugin(&mut self, plugin_id: &PluginId<'_>) -> Result<()> {
        let rc_file_path = self.rc_file_path(plugin_id);
        let desktop_dir_path = self.desktop_dir_path(plugin_id);
        if rc_file_path.is_file() {
            self.log(PatchEvent::RemovePluginRcFile {
                path: &rc_file_path,
            })
            .context("error logging remove plugin RC file")?;
            if !self.dry_run {
                fs::remove_file(rc_file_path)
                    .context("error removing RC file")?;
            }
        } else if desktop_dir_path.is_dir() {
            self.log(PatchEvent::RemovePluginDesktopDir {
                path: &desktop_dir_path,
            })
            .context("error logging remove plugin desktop dir")?;
            if !self.dry_run {
                fs::remove_dir_all(desktop_dir_path)
                    .context("error removing desktop dir")?;
            }
        } else {
            bail!("plugin {} does not exist", plugin_id)
        }
        Ok(())
    }

    fn create_desktop_dir(&mut self, plugin_id: &PluginId<'_>) -> Result<()> {
        let path = self.desktop_dir_path(plugin_id);
        self.log(PatchEvent::CreateDesktopDir { path: &path })
            .context("error logging create desktop dir")?;
        if !self.dry_run {
            fs::create_dir(path).context("error creating desktop dir")?;
        }
        Ok(())
    }

    fn link_desktop_file(
        &mut self,
        plugin_id: &PluginId<'_>,
        desktop_id: u64,
        target_path: &Path,
    ) -> Result<()> {
        let path = self.desktop_file_path(plugin_id, desktop_id);
        self.log(PatchEvent::LinkDesktopFile {
            path: &path,
            target_path,
        })
        .context("error logging link desktop file")?;
        if !self.dry_run {
            {
                cfg_if! {
                    if #[cfg(unix)] {
                        std::os::unix::fs::symlink(target_path, path)
                            .map_err(anyhow::Error::from)
                    } else {
                        anyhow!("platform does support FS linking")
                    }
                }
            }
            .context("error linking desktop file")?;
        }
        Ok(())
    }

    fn remove_desktop_file(
        &mut self,
        plugin_id: &PluginId<'_>,
        desktop_id: u64,
    ) -> Result<()> {
        let path = self.desktop_file_path(plugin_id, desktop_id);
        self.log(PatchEvent::RemoveDesktopFile { path: &path })
            .context("error logging remove desktop file")?;
        if !self.dry_run {
            fs::remove_file(path).context("error removing desktop file")?;
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PatchEvent<'a> {
    #[serde(rename_all = "kebab-case")]
    Cfg { content: &'a Cfg<'a> },
    #[serde(rename_all = "kebab-case")]
    RemovePluginRcFile { path: &'a Path },
    #[serde(rename_all = "kebab-case")]
    RemovePluginDesktopDir { path: &'a Path },
    #[serde(rename_all = "kebab-case")]
    CreateDesktopDir { path: &'a Path },
    #[serde(rename_all = "kebab-case")]
    LinkDesktopFile {
        path: &'a Path,
        target_path: &'a Path,
    },
    #[serde(rename_all = "kebab-case")]
    RemoveDesktopFile { path: &'a Path },
}

impl PanelPatch<'_> {
    pub fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        self.plugin_configs.apply(applier)?;
        Ok(())
    }
}

impl PluginConfigsPatch<'_> {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        for plugin_config_patch in self.0.changed.into_values() {
            plugin_config_patch.apply(applier)?;
        }
        for plugin_config in self.0.added.into_values() {
            plugin_config.apply(applier)?;
        }
        for id in self.0.removed {
            applier.remove_plugin(&id)?;
        }
        Ok(())
    }
}

impl PluginConfig<'_> {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        match self.file {
            PluginConfigFile::Rc(cfg) => {
                cfg.apply(&mut applier.rc_cfg_applier(&self.id))
            },
            PluginConfigFile::DesktopDir(desktop_dir) => {
                applier.create_desktop_dir(&self.id)?;
                for file in desktop_dir.files.0.into_values() {
                    file.apply(applier, &self.id)?;
                }
                Ok(())
            },
        }
    }
}

impl DesktopFile<'_> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        plugin_id: &PluginId<'_>,
    ) -> Result<()> {
        match self.content {
            DesktopFileContent::Cfg(cfg) => {
                cfg.apply(&mut applier.desktop_cfg_applier(plugin_id, self.id))
            },
            DesktopFileContent::Link(link) => {
                applier.link_desktop_file(plugin_id, self.id, &*link.path)
            },
        }
    }
}

impl PluginConfigPatch<'_> {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        match self {
            Self::Rc(rc_patch) => rc_patch.apply(applier),
            Self::DesktopDir(desktop_dir_patch) => {
                desktop_dir_patch.apply(applier)
            },
            Self::Changed(plugin_config) => {
                applier.remove_plugin(&plugin_config.id)?;
                plugin_config.apply(applier)?;
                Ok(())
            },
        }
    }
}

impl RcPatch<'_> {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        self.cfg.apply(&mut applier.rc_cfg_applier(&self.id))?;
        Ok(())
    }
}

impl DesktopDirPatch<'_> {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        for desktop_file_patch in self.files.changed.into_values() {
            desktop_file_patch.apply(applier, &self.id)?;
        }
        for desktop_file in self.files.added.into_values() {
            desktop_file.apply(applier, &self.id)?;
        }
        for id in self.files.removed {
            applier.remove_desktop_file(&self.id, id)?;
        }
        Ok(())
    }
}

impl DesktopFilePatch<'_> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        plugin_id: &PluginId<'_>,
    ) -> Result<()> {
        match self {
            Self::Cfg(desktop_file_cfg_patch) => {
                desktop_file_cfg_patch.apply(applier, plugin_id)
            },
            Self::Link(link_patch) => link_patch.apply(applier, plugin_id),
            Self::Changed(desktop_file) => {
                desktop_file.apply(applier, plugin_id)
            },
        }
    }
}

impl DesktopFileCfgPatch<'_> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        plugin_id: &PluginId<'_>,
    ) -> Result<()> {
        self.cfg
            .apply(&mut applier.desktop_cfg_applier(plugin_id, self.id))
    }
}

impl LinkPatch<'_> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        plugin_id: &PluginId<'_>,
    ) -> Result<()> {
        if let Some(path) = self.path {
            applier.remove_desktop_file(plugin_id, self.id)?;
            applier.link_desktop_file(plugin_id, self.id, &*path)?;
        }
        Ok(())
    }
}
