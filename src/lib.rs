#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

mod cfg;
mod dbus;
mod gtk;
mod panel;
mod serde;
mod xfconf;

pub use xfconf::ClearPath;

use ::serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use dbus::DBus;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

// TODO: make all config parts optional in deserialize

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct XfceConfig<'a> {
    xfconf: xfconf::Xfconf<'a>,
    panel: panel::Panel<'a>,
    gtk: gtk::Gtk<'a>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct XfceConfigPatch<'a> {
    #[serde(skip_serializing_if = "xfconf::XfconfPatch::is_empty")]
    xfconf: xfconf::XfconfPatch<'a>,
    #[serde(skip_serializing_if = "panel::PanelPatch::is_empty")]
    panel: panel::PanelPatch<'a>,
    #[serde(skip_serializing_if = "gtk::GtkPatch::is_empty")]
    gtk: gtk::GtkPatch<'a>,
}

impl<'a> XfceConfigPatch<'a> {
    pub fn diff(
        old: XfceConfig<'a>,
        new: XfceConfig<'a>,
        clear_paths: &[ClearPath<'_>],
    ) -> Self {
        XfceConfigPatch {
            xfconf: xfconf::XfconfPatch::diff(
                old.xfconf,
                new.xfconf,
                clear_paths,
            ),
            panel: panel::PanelPatch::diff(old.panel, new.panel),
            gtk: gtk::GtkPatch::diff(old.gtk, new.gtk),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.xfconf.is_empty() && self.panel.is_empty() && self.gtk.is_empty()
    }
}

impl XfceConfig<'static> {
    pub fn from_json_reader<R>(reader: R) -> Result<Self>
    where
        R: Read,
    {
        serde_json::from_reader(reader).map_err(Into::into)
    }

    pub fn from_env(
        xfce4_config_dir: &Path,
        gtk_config_dir: &Path,
    ) -> Result<Self> {
        let xfconf = xfconf::Xfconf::read(&xfce4_config_dir.join("xfconf"))
            .context("error loading xfconf data")?;
        let panel = panel::Panel::read(&xfce4_config_dir.join("panel"))
            .context("error loading panel data")?;
        let gtk =
            gtk::Gtk::read(gtk_config_dir).context("error loading gtk data")?;
        Ok(Self { xfconf, panel, gtk })
    }
}

pub struct Applier {
    dry_run: bool,
    patch_recorder: PatchRecorder,
    xfce4_config_dir: PathBuf,
    gtk_config_dir: PathBuf,
}

struct PatchRecorder {
    file: fs::File,
}

impl Applier {
    pub fn new(
        dry_run: bool,
        log_dir: &Path,
        xfce4_config_dir: PathBuf,
        gtk_config_dir: PathBuf,
    ) -> Result<Self> {
        let patch_recorder = PatchRecorder::new(&log_dir.join("patches.json"))
            .context("error creating patch recorder")?;
        Ok(Self {
            dry_run,
            patch_recorder,
            xfce4_config_dir,
            gtk_config_dir,
        })
    }
}

impl XfceConfigPatch<'_> {
    pub fn apply(self, applier: &mut Applier) -> Result<()> {
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
                applier.xfce4_config_dir.join("panel"),
            ))
            .context("error applying panel")?;
        self.gtk
            .apply(&mut gtk::Applier::new(
                applier.dry_run,
                &mut applier.patch_recorder,
                applier.gtk_config_dir.clone(),
            ))
            .context("error applying gtk")?;

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
    Channel(xfconf::PatchEvent<'a>),
    Panel(panel::PatchEvent<'a>),
    #[serde(rename_all = "kebab-case")]
    Cfg {
        content: &'a cfg::Cfg<'a>,
    },
}
