use anyhow::{bail, Result};
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::HashMap,
    io::{BufRead, Write},
};

#[derive(Debug, Default, Deserialize)]
pub struct Cfg<'a> {
    pub root: HashMap<Cow<'a, str>, Cow<'a, str>>,
    pub sections: HashMap<Cow<'a, str>, HashMap<Cow<'a, str>, Cow<'a, str>>>,
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
