use anyhow::{bail, Result};
use std::io::{BufRead, Write};

#[derive(Debug, Default)]
pub struct Cfg {
    pub root_props: Vec<(String, String)>,
    pub sections: Vec<(String, Vec<(String, String)>)>,
}

impl Cfg {
    pub fn read<R>(reader: R) -> Result<Self>
    where
        R: BufRead,
    {
        let mut cfg = Self::default();
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                // ignore
            } else if let Some(line) = line.strip_prefix('[') {
                if let Some(line) = line.strip_suffix(']') {
                    cfg.sections.push((line.to_owned(), Vec::new()));
                } else {
                    bail!("section name missing trailing bracket");
                }
            } else if let Some((key, value)) = line.split_once('=') {
                cfg.sections
                    .last_mut()
                    .map_or(
                        &mut cfg.root_props,
                        |(_section_name, section_props)| section_props,
                    )
                    .push((key.to_owned(), value.to_owned()));
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

        for (key, value) in &self.root_props {
            write_prop(&mut writer, key, value)?;
        }
        if !self.root_props.is_empty() {
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
