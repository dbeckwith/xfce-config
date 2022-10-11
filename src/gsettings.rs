use crate::{serde::IdMap, PatchRecorder};
use anyhow::{Context, Result};
use gio::prelude::{SettingsExt, SettingsExtManual};
use serde::{de, ser, Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GSettings {
    #[serde(default, skip_serializing_if = "Schemas::is_empty")]
    schemas: Schemas,
}

impl GSettings {
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Schemas(IdMap<Schema>);

impl Schemas {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Schema {
    id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    values: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
struct Value(glib::Variant);

impl GSettings {
    pub fn load(new_gsettings: &Self) -> Result<Self> {
        let schemas = Schemas::load(&new_gsettings.schemas)?;
        Ok(Self { schemas })
    }
}

impl Schemas {
    fn load(new_schemas: &Self) -> Result<Self> {
        let schemas = (new_schemas.0)
            .0
            .keys()
            .into_iter()
            .map(|schema_id| {
                Schema::load(schema_id.clone()).with_context(|| {
                    format!("error loading schema {}", schema_id)
                })
            })
            .collect::<Result<IdMap<_>>>()?;
        Ok(Self(schemas))
    }
}

impl crate::serde::Id for Schema {
    type Id = String;

    fn id(&self) -> &Self::Id {
        &self.id
    }
}

impl Schema {
    fn load(id: String) -> Result<Self> {
        let settings = gio::Settings::new(&id);
        let settings_schema = settings
            .settings_schema()
            .context("error getting settings schema object")?;
        let values = settings_schema
            .list_keys()
            .into_iter()
            .map(|key| {
                let value = settings.value(&key);
                Ok((key.to_string(), Value(value)))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        Ok(Self { id, values })
    }
}

impl<'de> de::Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Value;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "glib variant string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                glib::Variant::parse(None, v).map(Value).map_err(|error| {
                    E::custom(format_args!(
                        "error parsing glib variant: {}",
                        error
                    ))
                })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl ser::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(self.0.print(false).as_str())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct GSettingsPatch {
    #[serde(skip_serializing_if = "SchemasPatch::is_empty")]
    schemas: SchemasPatch,
}

impl GSettingsPatch {
    pub fn diff(old: GSettings, new: GSettings) -> Self {
        Self {
            schemas: SchemasPatch::diff(old.schemas, new.schemas),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct SchemasPatch {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    changed: BTreeMap<String, SchemaPatch>,
}

impl SchemasPatch {
    fn diff(mut old: Schemas, new: Schemas) -> Self {
        let mut changed = BTreeMap::new();
        for (key, new_value) in (new.0).0.into_iter() {
            if let Some(old_value) = (old.0).0.remove(&key) {
                let patch = SchemaPatch::diff(old_value, new_value);
                if !patch.is_empty() {
                    changed.insert(key, patch);
                }
            }
        }
        Self { changed }
    }

    fn is_empty(&self) -> bool {
        self.changed.is_empty()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct SchemaPatch {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    changed: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    added: BTreeMap<String, Value>,
}

impl SchemaPatch {
    fn diff(mut old: Schema, new: Schema) -> Self {
        let mut changed = BTreeMap::new();
        let mut added = BTreeMap::new();
        for (key, new_value) in new.values.into_iter() {
            if let Some(old_value) = old.values.remove(&key) {
                if old_value != new_value {
                    changed.insert(key, new_value);
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
}

pub struct Applier<'a> {
    dry_run: bool,
    patch_recorder: &'a mut PatchRecorder,
}

struct SchemaApplier<'a, 'b> {
    applier: &'a mut Applier<'b>,
    id: &'a str,
    settings: gio::Settings,
}

impl<'a> Applier<'a> {
    pub(crate) fn new(
        dry_run: bool,
        patch_recorder: &'a mut PatchRecorder,
    ) -> Self {
        Self {
            dry_run,
            patch_recorder,
        }
    }
}

impl<'a, 'b> SchemaApplier<'a, 'b> {
    fn new(applier: &'a mut Applier<'b>, id: &'a str) -> Self {
        let settings = gio::Settings::new(id);
        settings.delay();
        Self {
            applier,
            id,
            settings,
        }
    }

    fn set(&mut self, key: &str, value: Value) -> Result<()> {
        self.applier
            .patch_recorder
            .log(&crate::PatchEvent::GSettings(PatchEvent::Set {
                schema_id: self.id,
                key,
                value: value.0.print(false).to_string(),
            }))
            .context("error logging gsettings set")?;
        if !self.applier.dry_run {
            self.settings.set(key, &value.0).with_context(|| {
                format!(
                    "error setting gsettings value for schema {} and key {}",
                    self.id, key
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for SchemaApplier<'_, '_> {
    fn drop(&mut self) {
        self.settings.apply()
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PatchEvent<'a> {
    #[serde(rename_all = "kebab-case")]
    Set {
        schema_id: &'a str,
        key: &'a str,
        value: String,
    },
}

impl GSettingsPatch {
    pub fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        self.schemas.apply(applier)?;
        gio::Settings::sync();
        Ok(())
    }
}

impl SchemasPatch {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        for (id, schema) in self.changed.into_iter() {
            let mut applier = SchemaApplier::new(applier, &id);
            schema.apply(&mut applier)?;
        }
        Ok(())
    }
}

impl SchemaPatch {
    fn apply(self, applier: &mut SchemaApplier<'_, '_>) -> Result<()> {
        for (key, value) in self.changed.into_iter() {
            applier.set(&key, value)?;
        }
        for (key, value) in self.added.into_iter() {
            applier.set(&key, value)?;
        }
        Ok(())
    }
}
