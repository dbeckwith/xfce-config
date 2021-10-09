use crate::{
    cfg::{Applier as CfgApplier, Cfg, CfgPatch},
    PatchRecorder,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Gtk<'a> {
    settings: Settings<'a>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Settings<'a>(Option<Cfg<'a>>);

impl Gtk<'static> {
    pub fn read(dir: &Path) -> Result<Self> {
        let settings = Settings::read(dir)?;
        Ok(Self { settings })
    }
}

impl Settings<'static> {
    pub fn read(dir: &Path) -> Result<Self> {
        let file = match fs::File::open(dir.join("settings.ini")) {
            Ok(file) => Ok(Some(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
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
pub struct GtkPatch<'a> {
    #[serde(skip_serializing_if = "SettingsPatch::is_empty")]
    settings: SettingsPatch<'a>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
enum SettingsPatch<'a> {
    Added(Cfg<'a>),
    Changed(CfgPatch<'a>),
    Unchanged,
}

impl<'a> GtkPatch<'a> {
    pub fn diff(old: Gtk<'a>, new: Gtk<'a>) -> Self {
        Self {
            settings: SettingsPatch::diff(old.settings, new.settings),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }
}

impl<'a> SettingsPatch<'a> {
    fn diff(old: Settings<'a>, new: Settings<'a>) -> Self {
        match (old.0, new.0) {
            (Some(old_content), Some(new_content)) => {
                Self::Changed(CfgPatch::diff(old_content, new_content))
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

impl GtkPatch<'_> {
    pub fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        self.settings.apply(applier)?;
        Ok(())
    }
}

impl SettingsPatch<'_> {
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
