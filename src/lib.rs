#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

mod cfg;
mod dbus;
mod general;
mod gtk;
mod json;
mod panel;
mod serde;
mod xfconf;

use ::serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use dbus::DBus;
use std::{
    borrow::Cow,
    fs,
    io::{self, Read, Write},
    path::Path,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct XfceConfig {
    #[serde(default, skip_serializing_if = "xfconf::Xfconf::is_empty")]
    xfconf: xfconf::Xfconf,
    #[serde(default, skip_serializing_if = "panel::Panel::is_empty")]
    panel: panel::Panel,
    #[serde(default, skip_serializing_if = "gtk::Gtk::is_empty")]
    gtk: gtk::Gtk,
    #[serde(default, skip_serializing_if = "general::General::is_empty")]
    general: general::General,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct XfceConfigPatch {
    #[serde(skip_serializing_if = "xfconf::XfconfPatch::is_empty")]
    xfconf: xfconf::XfconfPatch,
    #[serde(skip_serializing_if = "panel::PanelPatch::is_empty")]
    panel: panel::PanelPatch,
    #[serde(skip_serializing_if = "gtk::GtkPatch::is_empty")]
    gtk: gtk::GtkPatch,
    #[serde(skip_serializing_if = "general::GeneralPatch::is_empty")]
    general: general::GeneralPatch,
}

impl XfceConfigPatch {
    pub fn diff(old: XfceConfig, new: XfceConfig) -> Result<Self> {
        Ok(XfceConfigPatch {
            xfconf: xfconf::XfconfPatch::diff(old.xfconf, new.xfconf),
            panel: panel::PanelPatch::diff(old.panel, new.panel),
            gtk: gtk::GtkPatch::diff(old.gtk, new.gtk),
            general: general::GeneralPatch::diff(old.general, new.general)
                .context("error diffing general")?,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.xfconf.is_empty() && self.panel.is_empty() && self.gtk.is_empty()
    }
}

impl XfceConfig {
    pub fn from_json_reader<R>(reader: R) -> Result<Self>
    where
        R: Read,
    {
        serde_json::from_reader(reader).map_err(Into::into)
    }

    pub fn from_env(
        new_config: &Self,
        config_dir: &Path,
        xfce4_config_dir: &Path,
        gtk_config_dir: &Path,
    ) -> Result<Self> {
        // TODO: consider new_config.xfconf to only load used channels
        let xfconf =
            xfconf::Xfconf::load().context("error loading xfconf data")?;
        let panel = panel::Panel::read(&xfce4_config_dir.join("panel"))
            .context("error loading panel data")?;
        let gtk =
            gtk::Gtk::read(gtk_config_dir).context("error loading gtk data")?;
        let general = general::General::read(&new_config.general, config_dir)
            .context("error loading general data")?;
        Ok(Self {
            xfconf,
            panel,
            gtk,
            general,
        })
    }
}

pub struct Applier<'a> {
    dry_run: bool,
    patch_recorder: PatchRecorder,
    xfce4_config_dir: Cow<'a, Path>,
    gtk_config_dir: Cow<'a, Path>,
    config_dir: Cow<'a, Path>,
}

struct PatchRecorder {
    file: fs::File,
}

impl<'a> Applier<'a> {
    pub fn new(
        dry_run: bool,
        log_dir: &Path,
        xfce4_config_dir: Cow<'a, Path>,
        gtk_config_dir: Cow<'a, Path>,
        config_dir: Cow<'a, Path>,
    ) -> Result<Self> {
        let patch_recorder = PatchRecorder::new(&log_dir.join("patches.json"))
            .context("error creating patch recorder")?;
        Ok(Self {
            dry_run,
            patch_recorder,
            xfce4_config_dir,
            gtk_config_dir,
            config_dir,
        })
    }
}

impl XfceConfigPatch {
    pub fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        let panel_config_changed =
            !self.panel.is_empty() || self.xfconf.has_panel_changes();

        self.xfconf
            .apply(
                &mut xfconf::Applier::new(
                    applier.dry_run,
                    &mut applier.patch_recorder,
                )
                .context("error creating xfconf applier")?,
            )
            .context("error applying xfconf")?;
        self.panel
            .apply(&mut panel::Applier::new(
                applier.dry_run,
                &mut applier.patch_recorder,
                applier.xfce4_config_dir.join("panel").into(),
            ))
            .context("error applying panel")?;
        self.gtk
            .apply(&mut gtk::Applier::new(
                applier.dry_run,
                &mut applier.patch_recorder,
                applier.gtk_config_dir.clone(),
            ))
            .context("error applying gtk")?;
        self.general
            .apply(&mut general::Applier::new(
                applier.dry_run,
                &mut applier.patch_recorder,
                applier.config_dir.clone(),
            ))
            .context("error applying general")?;

        // restart panel if its config changed
        if panel_config_changed && !applier.dry_run {
            DBus::new("org.xfce.Panel", "/org/xfce/Panel")?
                .call("Terminate", (true,))
                .context("error restarting panel")?;
        }

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
    Channel(xfconf::PatchEvent),
    Panel(panel::PatchEvent<'a>),
    #[serde(rename_all = "kebab-case")]
    Cfg {
        content: &'a cfg::Cfg,
    },
    #[serde(rename_all = "kebab-case")]
    Json {
        content: &'a json::Json,
    },
}

fn open_file(path: impl AsRef<Path>) -> io::Result<Option<fs::File>> {
    match fs::File::open(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if matches!(error.kind(), io::ErrorKind::NotFound) => {
            Ok(None)
        },
        Err(error) => Err(error),
    }
}
