use anyhow::Result;
use std::io::{BufRead, Write};

#[derive(Debug, Default)]
pub struct Cfg {
    pub root_props: Vec<(String, String)>,
    pub sections: Vec<(String, Vec<(String, String)>)>,
}

impl Cfg {
    pub fn read<R>(_reader: R) -> Result<Self>
    where
        R: BufRead,
    {
        todo!()
    }

    pub fn write<W>(&self, mut writer: W) -> Result<()>
    where
        W: Write,
    {
        fn write_prop<W>(writer: &mut W, key: &str, value: &str) -> Result<()>
        where
            W: Write,
        {
            write!(writer, "{}={}", key, value)?;
            Ok(())
        }

        for (key, value) in &self.root_props {
            write_prop(&mut writer, key, value)?;
        }
        for (section_name, props) in &self.sections {
            write!(&mut writer, "[{}]", section_name)?;
            for (key, value) in props {
                write_prop(&mut writer, key, value)?;
            }
        }
        Ok(())
    }
}
