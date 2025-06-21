use crate::PatchRecorder;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Json(Value);

impl Json {
    pub fn read<R>(reader: R) -> Result<Self>
    where
        R: Read,
    {
        serde_json::from_reader(reader)
            .map(Self)
            .map_err(Into::into)
    }

    pub fn write<W>(&self, writer: W) -> Result<()>
    where
        W: Write,
    {
        serde_json::to_writer(writer, &self.0).map_err(Into::into)
    }
}

#[derive(Debug, Serialize)]
pub struct JsonPatch {
    value: ValuePatch,
}

impl JsonPatch {
    pub fn diff(old: Json, new: Json) -> Self {
        Self {
            value: ValuePatch::diff(old.0, new.0),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    fn apply_to_old(self, old: &mut Json) {
        self.value.apply_to_old(&mut old.0);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ValuePatch {
    Null,
    Bool(SimplePatch<bool>),
    Number(SimplePatch<Number>),
    String(SimplePatch<String>),
    Array(SimplePatch<Vec<Value>>),
    Object(ObjectPatch),
    Changed(Value),
}

impl ValuePatch {
    fn diff(old: Value, new: Value) -> Self {
        match (old, new) {
            (Value::Null, Value::Null) => Self::Null,
            (Value::Bool(old), Value::Bool(new)) => {
                Self::Bool(SimplePatch::diff(old, new))
            },
            (Value::Number(old), Value::Number(new)) => {
                Self::Number(SimplePatch::diff(old, new))
            },
            (Value::String(old), Value::String(new)) => {
                Self::String(SimplePatch::diff(old, new))
            },
            (Value::Array(old), Value::Array(new)) => {
                Self::Array(SimplePatch::diff(old, new))
            },
            (Value::Object(old), Value::Object(new)) => {
                Self::Object(ObjectPatch::diff(old, new))
            },
            (_old, new) => Self::Changed(new),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            ValuePatch::Null => true,
            ValuePatch::Bool(patch) => patch.is_empty(),
            ValuePatch::Number(patch) => patch.is_empty(),
            ValuePatch::String(patch) => patch.is_empty(),
            ValuePatch::Array(patch) => patch.is_empty(),
            ValuePatch::Object(patch) => patch.is_empty(),
            ValuePatch::Changed(_) => false,
        }
    }

    fn apply_to_old(self, old: &mut Value) {
        match (self, old) {
            (ValuePatch::Null, Value::Null) => {},
            (ValuePatch::Bool(patch), Value::Bool(old)) => {
                patch.apply_to_old(old);
            },
            (ValuePatch::Number(patch), Value::Number(old)) => {
                patch.apply_to_old(old);
            },
            (ValuePatch::String(patch), Value::String(old)) => {
                patch.apply_to_old(old);
            },
            (ValuePatch::Array(patch), Value::Array(old)) => {
                patch.apply_to_old(old);
            },
            (ValuePatch::Object(patch), Value::Object(old)) => {
                patch.apply_to_old(old);
            },
            (ValuePatch::Changed(value), old) => {
                *old = value;
            },
            _ => unreachable!("value type does not match patch type"),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ObjectPatch {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    changed: BTreeMap<String, ValuePatch>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    added: BTreeMap<String, Value>,
}

impl ObjectPatch {
    fn diff(mut old: Map<String, Value>, new: Map<String, Value>) -> Self {
        let mut changed = BTreeMap::new();
        let mut added = BTreeMap::new();
        for (key, new_value) in new.into_iter() {
            if let Some(old_value) = old.remove(&key) {
                let patch = ValuePatch::diff(old_value, new_value);
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

    fn apply_to_old(self, old: &mut Map<String, Value>) {
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
#[serde(rename_all = "kebab-case")]
struct SimplePatch<T> {
    value: Option<T>,
}

impl<T> SimplePatch<T>
where
    T: PartialEq,
{
    fn diff(old: T, new: T) -> Self {
        Self {
            value: (old != new).then_some(new),
        }
    }

    fn is_empty(&self) -> bool {
        self.value.is_none()
    }

    fn apply_to_old(self, old: &mut T) {
        if let Some(value) = self.value {
            *old = value;
        }
    }
}

pub struct Applier<'a> {
    dry_run: bool,
    patch_recorder: &'a mut PatchRecorder,
    path: Cow<'a, Path>,
}

impl<'a> Applier<'a> {
    pub(crate) fn new(
        dry_run: bool,
        patch_recorder: &'a mut PatchRecorder,
        path: Cow<'a, Path>,
    ) -> Self {
        Self {
            dry_run,
            patch_recorder,
            path,
        }
    }

    fn write_json(&mut self, json: &Json) -> Result<()> {
        self.patch_recorder
            .log(&crate::PatchEvent::Json { content: json })
            .context("error logging JSON write")?;
        if !self.dry_run {
            let mut tmp = self.path.clone().into_owned().into_os_string();
            tmp.push(".new");
            let tmp = PathBuf::from(tmp);
            json.write(
                fs::File::create(&tmp)
                    .context("error creating temporary JSON file")?,
            )
            .context("error writing temporary JSON file")?;
            fs::rename(tmp, &self.path)
                .context("error moving temporary JSON file")?;
        }
        Ok(())
    }

    fn update_json(&mut self, json_patch: JsonPatch) -> Result<()> {
        // TODO: remove double read of existing file
        // instead of reading it here, the patch should keep the old data
        let mut json = Json::read(
            fs::File::open(&self.path)
                .map(io::BufReader::new)
                .context("error opening existing JSON file")?,
        )
        .context("error reading existing JSON file")?;
        json_patch.apply_to_old(&mut json);
        self.write_json(&json)?;
        Ok(())
    }
}

impl Json {
    pub fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        applier.write_json(&self)?;
        Ok(())
    }
}

impl JsonPatch {
    pub fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        applier.update_json(self)?;
        Ok(())
    }
}
