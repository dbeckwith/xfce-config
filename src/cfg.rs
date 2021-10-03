use anyhow::{bail, Result};
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::BTreeMap,
    io::{BufRead, Write},
};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Cfg<'a> {
    #[serde(default)]
    pub root: BTreeMap<Cow<'a, str>, Cow<'a, str>>,
    #[serde(default)]
    pub sections: BTreeMap<Cow<'a, str>, BTreeMap<Cow<'a, str>, Cow<'a, str>>>,
}

#[derive(Debug)]
pub struct CfgPatch<'a> {
    root: MapPatch<'a, StrPatch<'a>>,
    sections: MapPatch<'a, MapPatch<'a, StrPatch<'a>>>,
}

impl<'a> CfgPatch<'a> {
    pub fn diff(old: &Cfg<'a>, new: &Cfg<'a>) -> Self {
        Self {
            root: MapPatch::diff(&old.root, &new.root),
            sections: MapPatch::diff(&old.sections, &new.sections),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_empty() && self.sections.is_empty()
    }
}

trait Patch {
    type Data;

    fn diff(old: &Self::Data, new: &Self::Data) -> Self;

    fn is_empty(&self) -> bool;
}

#[derive(Debug)]
struct MapPatch<'a, T>
where
    T: Patch,
{
    changed: BTreeMap<Cow<'a, str>, T>,
    added: BTreeMap<Cow<'a, str>, T::Data>,
}

impl<'a, T> Patch for MapPatch<'a, T>
where
    T: Patch,
    T::Data: Clone,
{
    type Data = BTreeMap<Cow<'a, str>, T::Data>;

    fn diff(old: &Self::Data, new: &Self::Data) -> Self {
        let mut changed = BTreeMap::new();
        let mut added = BTreeMap::new();
        for (key, new_value) in new.iter() {
            if let Some(old_value) = old.get(key) {
                let patch = T::diff(old_value, new_value);
                if !patch.is_empty() {
                    changed.insert(key.clone(), patch);
                }
            } else {
                added.insert(key.clone(), new_value.clone());
            }
        }
        Self { changed, added }
    }

    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty()
    }
}

#[derive(Debug)]
struct StrPatch<'a> {
    value: Option<Cow<'a, str>>,
}

impl<'a> Patch for StrPatch<'a> {
    type Data = Cow<'a, str>;

    fn diff(old: &Self::Data, new: &Self::Data) -> Self {
        Self {
            value: (old != new).then(|| new.clone()),
        }
    }

    fn is_empty(&self) -> bool {
        self.value.is_none()
    }
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
