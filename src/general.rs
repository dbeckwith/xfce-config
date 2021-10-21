use crate::{
    cfg::{Applier as CfgApplier, Cfg, CfgPatch},
    json::{Applier as JsonApplier, Json, JsonPatch},
    open_file,
    serde::IdMap,
    PatchRecorder,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt,
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct General {
    #[serde(default, skip_serializing_if = "Configs::is_empty")]
    configs: Configs,
}

impl General {
    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Configs(IdMap<Config>);

impl Configs {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Config {
    #[serde(flatten)]
    id: ConfigId,
    #[serde(flatten)]
    content: ConfigContent,
    // TODO: support clear paths
}

impl crate::serde::Id for Config {
    type Id = ConfigId;

    fn id(&self) -> &Self::Id {
        &self.id
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
struct ConfigId {
    root: ConfigRoot,
    // TODO: assert that path is relative
    path: PathBuf,
}

impl fmt::Display for ConfigId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{{}}}{}{}",
            match self.root {
                ConfigRoot::Config => "config",
            },
            std::path::MAIN_SEPARATOR,
            self.path.display()
        )
    }
}

impl ConfigId {
    fn full_path(&self, config_dir: &Path) -> PathBuf {
        let root = match self.root {
            ConfigRoot::Config => config_dir,
        };
        root.join(&self.path)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
enum ConfigRoot {
    Config,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "kebab-case")]
enum ConfigContent {
    Cfg(Cfg),
    Json(Json),
}

impl General {
    pub fn read(new_general: &Self, config_dir: &Path) -> Result<Self> {
        let configs = Configs::read(&new_general.configs, config_dir)
            .context("error loading configs")?;
        Ok(Self { configs })
    }
}

impl Configs {
    fn read(new_configs: &Self, config_dir: &Path) -> Result<Self> {
        (new_configs.0)
            .0
            .values()
            .filter_map(|new_config| {
                let full_path = new_config.id.full_path(config_dir);
                let content =
                    match ConfigContent::read(full_path, &new_config.content) {
                        Ok(Some(content)) => content,
                        Ok(None) => return None,
                        Err(error) => return Some(Err(error)),
                    };
                let id = new_config.id.clone();
                Some(Ok(Config { id, content }))
            })
            .collect::<Result<IdMap<_>>>()
            .map(Self)
    }
}

impl ConfigContent {
    fn read(path: PathBuf, kind: &Self) -> Result<Option<Self>> {
        let file = match open_file(path).context("error opening config file")? {
            Some(file) => file,
            None => return Ok(None),
        };
        match kind {
            ConfigContent::Cfg(_) => Cfg::read(io::BufReader::new(file))
                .context("error reading CFG file")
                .map(ConfigContent::Cfg),
            ConfigContent::Json(_) => serde_json::from_reader(file)
                .context("error reading JSON file")
                .map(ConfigContent::Json),
        }
        .map(Some)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct GeneralPatch {
    #[serde(skip_serializing_if = "ConfigsPatch::is_empty")]
    configs: ConfigsPatch,
}

impl GeneralPatch {
    pub fn diff(old: General, new: General) -> Result<Self> {
        Ok(Self {
            configs: ConfigsPatch::diff(old.configs, new.configs)
                .context("error diffing configs")?,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ConfigsPatch {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    changed: BTreeMap<ConfigId, ConfigPatch>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    added: Vec<Config>,
}

impl ConfigsPatch {
    fn diff(mut old: Configs, new: Configs) -> Result<Self> {
        let mut changed = BTreeMap::new();
        let mut added = Vec::new();
        for (key, new_value) in (new.0).0.into_iter() {
            if let Some(old_value) = (old.0).0.remove(&key) {
                let patch = ConfigPatch::diff(old_value, new_value)
                    .with_context(|| {
                        format!("error diffing config for {}", key)
                    })?;
                if !patch.is_empty() {
                    changed.insert(key, patch);
                }
            } else {
                added.push(new_value);
            }
        }
        Ok(Self { changed, added })
    }

    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ConfigPatch {
    id: ConfigId,
    content: ConfigContentPatch,
}

impl ConfigPatch {
    fn diff(old: Config, new: Config) -> Result<Self> {
        Ok(Self {
            id: old.id,
            content: ConfigContentPatch::diff(old.content, new.content)?,
        })
    }

    fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ConfigContentPatch {
    Cfg(CfgPatch),
    Json(JsonPatch),
}

impl ConfigContentPatch {
    fn diff(old: ConfigContent, new: ConfigContent) -> Result<Self> {
        match (old, new) {
            (ConfigContent::Cfg(old), ConfigContent::Cfg(new)) => {
                Ok(Self::Cfg(CfgPatch::diff(old, new)))
            },
            (ConfigContent::Json(old), ConfigContent::Json(new)) => {
                Ok(Self::Json(JsonPatch::diff(old, new)))
            },
            _ => bail!("new config content type does not match existing type"),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            ConfigContentPatch::Cfg(cfg_patch) => cfg_patch.is_empty(),
            ConfigContentPatch::Json(json_patch) => json_patch.is_empty(),
        }
    }
}
pub struct Applier<'a> {
    dry_run: bool,
    patch_recorder: &'a mut PatchRecorder,
    config_dir: Cow<'a, Path>,
}

impl<'a> Applier<'a> {
    pub(crate) fn new(
        dry_run: bool,
        patch_recorder: &'a mut PatchRecorder,
        config_dir: Cow<'a, Path>,
    ) -> Self {
        Self {
            dry_run,
            patch_recorder,
            config_dir,
        }
    }

    fn cfg_applier(&mut self, id: &ConfigId) -> CfgApplier<'_> {
        CfgApplier::new(
            self.dry_run,
            self.patch_recorder,
            id.full_path(&self.config_dir).into(),
        )
    }

    fn json_applier(&mut self, id: &ConfigId) -> JsonApplier<'_> {
        JsonApplier::new(
            self.dry_run,
            self.patch_recorder,
            id.full_path(&self.config_dir).into(),
        )
    }
}

impl GeneralPatch {
    pub fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        self.configs.apply(applier)?;
        Ok(())
    }
}

impl ConfigsPatch {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        for config in self.added {
            config.apply(applier)?;
        }
        for cfg_patch in self.changed.into_values() {
            cfg_patch.apply(applier)?;
        }
        Ok(())
    }
}

impl Config {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        match self.content {
            ConfigContent::Cfg(cfg) => {
                cfg.apply(&mut applier.cfg_applier(&self.id))
            },
            ConfigContent::Json(json) => {
                json.apply(&mut applier.json_applier(&self.id))
            },
        }
    }
}

impl ConfigPatch {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        match self.content {
            ConfigContentPatch::Cfg(cfg_patch) => {
                cfg_patch.apply(&mut applier.cfg_applier(&self.id))
            },
            ConfigContentPatch::Json(json_patch) => {
                json_patch.apply(&mut applier.json_applier(&self.id))
            },
        }
    }
}
