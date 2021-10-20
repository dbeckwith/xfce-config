use crate::{
    cfg::{Applier as CfgApplier, Cfg, CfgPatch},
    open_file,
    PatchRecorder,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Gtk {
    #[serde(default, skip_serializing_if = "Settings::is_empty")]
    settings: Settings,
}

impl Gtk {
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Settings(Option<Cfg>);

impl Settings {
    fn is_empty(&self) -> bool {
        self.0.is_none()
    }
}

impl Gtk {
    pub fn read(dir: &Path) -> Result<Self> {
        let settings = Settings::read(dir)?;
        Ok(Self { settings })
    }
}

impl Settings {
    pub fn read(dir: &Path) -> Result<Self> {
        let file = open_file(dir.join("settings.ini"))
            .context("error opening GTK settings file")?;
        let content = file
            .map(|file| {
                let reader = io::BufReader::new(file);
                Cfg::read(reader).context("error reading GTK settings")
            })
            .transpose()?;
        Ok(Self(content))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct GtkPatch {
    #[serde(skip_serializing_if = "SettingsPatch::is_empty")]
    settings: SettingsPatch,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum SettingsPatch {
    Added(Cfg),
    Changed(CfgPatch),
    Unchanged,
}

impl GtkPatch {
    pub fn diff(old: Gtk, new: Gtk) -> Self {
        Self {
            settings: SettingsPatch::diff(old.settings, new.settings),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }
}

impl SettingsPatch {
    fn diff(old: Settings, new: Settings) -> Self {
        match (old.0, new.0) {
            (Some(old_content), Some(new_content)) => {
                let diff = CfgPatch::diff(old_content, new_content);
                if diff.is_empty() {
                    Self::Unchanged
                } else {
                    Self::Changed(diff)
                }
            },
            (None, Some(new_content)) => Self::Added(new_content),
            (_, None) => Self::Unchanged,
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Added(_cfg) => false,
            Self::Changed(cfg_patch) => cfg_patch.is_empty(),
            Self::Unchanged => true,
        }
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

    fn settings_applier(&mut self) -> CfgApplier<'_> {
        CfgApplier::new(
            self.dry_run,
            self.patch_recorder,
            self.dir.join("settings.ini"),
        )
    }

    fn ensure_dir(&mut self) -> Result<()> {
        if !self.dry_run {
            fs::create_dir_all(&self.dir)?;
        }
        Ok(())
    }
}

impl GtkPatch {
    pub fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        self.settings.apply(applier)?;
        Ok(())
    }
}

impl SettingsPatch {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        match self {
            Self::Added(cfg) => {
                applier.ensure_dir()?;
                cfg.apply(&mut applier.settings_applier())
            },
            Self::Changed(cfg_patch) => {
                cfg_patch.apply(&mut applier.settings_applier())
            },
            Self::Unchanged => Ok(()),
        }
    }
}
