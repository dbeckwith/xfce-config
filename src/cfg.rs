use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs,
    io::{self, BufRead, Write},
    path::PathBuf,
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Cfg<'a> {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub root: BTreeMap<Cow<'a, str>, Cow<'a, str>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sections: BTreeMap<Cow<'a, str>, BTreeMap<Cow<'a, str>, Cow<'a, str>>>,
}

impl Cfg<'_> {
    pub fn read<R>(reader: R) -> Result<Self>
    where
        R: BufRead,
    {
        let mut cfg = Self::default();
        let mut last_section = None;
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                // ignore
            } else if let Some(line) = line.strip_prefix('[') {
                if let Some(title) = line.strip_suffix(']') {
                    last_section = Some(
                        cfg.sections
                            .entry(title.to_owned().into())
                            .or_default(),
                    );
                } else {
                    bail!("section name missing trailing bracket");
                }
            } else if let Some((key, value)) = line.split_once('=') {
                last_section
                    .as_deref_mut()
                    .unwrap_or(&mut cfg.root)
                    .insert(key.to_owned().into(), value.to_owned().into());
            } else {
                bail!("line missing key-value separator");
            }
        }
        Ok(cfg)
    }

    pub fn write<W>(&self, mut writer: W) -> Result<()>
    where
        W: Write,
    {
        fn write_prop<W>(writer: &mut W, key: &str, value: &str) -> Result<()>
        where
            W: Write,
        {
            writeln!(writer, "{}={}", key, value)?;
            Ok(())
        }

        for (key, value) in &self.root {
            write_prop(&mut writer, key, value)?;
        }
        if !self.root.is_empty() {
            writeln!(&mut writer)?;
        }
        for (section_name, props) in &self.sections {
            writeln!(&mut writer, "[{}]", section_name)?;
            for (key, value) in props {
                write_prop(&mut writer, key, value)?;
            }
            writeln!(&mut writer)?;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct CfgPatch<'a> {
    #[serde(skip_serializing_if = "MapPatch::is_empty")]
    root: MapPatch<'a, StrPatch<'a>>,
    #[serde(skip_serializing_if = "MapPatch::is_empty")]
    sections: MapPatch<'a, MapPatch<'a, StrPatch<'a>>>,
}

impl<'a> CfgPatch<'a> {
    pub fn diff(old: Cfg<'a>, new: Cfg<'a>) -> Self {
        Self {
            root: MapPatch::diff(old.root, new.root),
            sections: MapPatch::diff(old.sections, new.sections),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_empty() && self.sections.is_empty()
    }

    fn apply_to_old(self, old: &mut Cfg<'a>) {
        self.root.apply_to_old(&mut old.root);
        self.sections.apply_to_old(&mut old.sections);
    }
}

trait Patch {
    type Data;

    fn diff(old: Self::Data, new: Self::Data) -> Self;

    fn is_empty(&self) -> bool;

    fn apply_to_old(self, old: &mut Self::Data);
}

#[derive(Debug, Serialize)]
#[serde(bound(serialize = "T: Patch + Serialize, T::Data: Serialize"))]
struct MapPatch<'a, T>
where
    T: Patch,
{
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    changed: BTreeMap<Cow<'a, str>, T>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    added: BTreeMap<Cow<'a, str>, T::Data>,
}

impl<'a, T> Patch for MapPatch<'a, T>
where
    T: Patch,
{
    type Data = BTreeMap<Cow<'a, str>, T::Data>;

    fn diff(mut old: Self::Data, new: Self::Data) -> Self {
        let mut changed = BTreeMap::new();
        let mut added = BTreeMap::new();
        for (key, new_value) in new.into_iter() {
            if let Some(old_value) = old.remove(&key) {
                let patch = T::diff(old_value, new_value);
                if !patch.is_empty() {
                    changed.insert(key, patch);
                }
            } else {
                added.insert(key, new_value);
            }
        }
        Self { changed, added }
    }

    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty()
    }

    fn apply_to_old(self, old: &mut Self::Data) {
        for (key, value_patch) in self.changed {
            if let Some(old_value) = old.get_mut(&key) {
                value_patch.apply_to_old(old_value);
            }
        }
        for (key, value) in self.added {
            old.insert(key, value);
        }
    }
}

#[derive(Debug, Serialize)]
struct StrPatch<'a> {
    value: Option<Cow<'a, str>>,
}

impl<'a> Patch for StrPatch<'a> {
    type Data = Cow<'a, str>;

    fn diff(old: Self::Data, new: Self::Data) -> Self {
        Self {
            value: (old != new).then(|| new),
        }
    }

    fn is_empty(&self) -> bool {
        self.value.is_none()
    }

    fn apply_to_old(self, old: &mut Self::Data) {
        if let Some(value) = self.value {
            *old = value;
        }
    }
}

pub struct Applier {
    dry_run: bool,
    path: PathBuf,
}

impl Applier {
    pub fn new(dry_run: bool, path: PathBuf) -> Self {
        Self { dry_run, path }
    }

    fn write_cfg(&mut self, cfg: &Cfg<'_>) -> Result<()> {
        eprintln!("writing CFG to {}:", self.path.display());
        eprintln!("====================");
        cfg.write(std::io::stderr())
            .context("error writing CFG to stderr")?;
        eprintln!("====================");
        if !self.dry_run {
            cfg.write(
                fs::File::create(&self.path)
                    .context("error creating CFG file")?,
            )
            .context("error writing CFG file")?;
        }
        Ok(())
    }

    fn update_cfg(&mut self, cfg_patch: CfgPatch<'_>) -> Result<()> {
        eprintln!("updating CFG at {}", self.path.display());
        let mut cfg = Cfg::read(
            fs::File::open(&self.path)
                .map(io::BufReader::new)
                .context("error opening existing CFG file")?,
        )
        .context("error reading existing CFG file")?;
        cfg_patch.apply_to_old(&mut cfg);
        self.write_cfg(&cfg)?;
        Ok(())
    }
}

impl Cfg<'_> {
    pub fn apply(self, applier: &mut Applier) -> Result<()> {
        applier.write_cfg(&self)?;
        Ok(())
    }
}

impl CfgPatch<'_> {
    pub fn apply(self, applier: &mut Applier) -> Result<()> {
        applier.update_cfg(self)?;
        Ok(())
    }
}
