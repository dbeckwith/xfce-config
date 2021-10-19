use crate::{dbus::DBus, serde::IdMap, PatchRecorder};
use anyhow::{anyhow, bail, Context, Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{btree_map, BTreeMap, BTreeSet},
    iter,
};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Xfconf<'a> {
    #[serde(default, skip_serializing_if = "Channels::is_empty")]
    channels: Channels<'a>,
}

impl Xfconf<'_> {
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Channels<'a>(IdMap<Channel<'a>>);

impl Channels<'_> {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Channel<'a> {
    name: Cow<'a, str>,
    #[serde(default, skip_serializing_if = "Properties::is_empty")]
    props: Properties<'a>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Properties<'a>(BTreeMap<Cow<'a, str>, Value<'a>>);

impl Properties<'_> {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Value<'a> {
    #[serde(flatten)]
    value: TypedValue<'a>,
    #[serde(default, skip_serializing_if = "Properties::is_empty")]
    props: Properties<'a>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
enum TypedValue<'a> {
    Bool(bool),
    Int(i32),
    Uint(u32),
    Double(f64),
    String(Cow<'a, str>),
    Array(Vec<Value<'a>>),
    Empty,
}

#[derive(Debug)]
pub struct ClearPath<'a> {
    channel: Cow<'a, str>,
    parts: Vec<ClearPathPart<'a>>,
    props: ClearPathProps<'a>,
}

#[derive(Debug)]
struct ClearPathPart<'a> {
    prop: Cow<'a, str>,
    prefix: bool,
}

#[derive(Debug)]
struct ClearPathProps<'a> {
    value_changed: bool,
    prefix: Option<Cow<'a, str>>,
}

impl Xfconf<'_> {
    pub fn load() -> Result<Self> {
        Ok(Self {
            channels: Channels::load().context("error loading channels")?,
        })
    }
}

impl Channels<'_> {
    fn load() -> Result<Self> {
        let mut dbus = DBus::new("org.xfce.Xfconf", "/org/xfce/Xfconf")?;
        let channels = dbus
            .call_no_args("ListChannels")?
            .try_child_value(0)
            .context("ListChannels had empty return value")?;

        fn value_from_variant(
            variant: glib::Variant,
        ) -> Result<TypedValue<'static>> {
            variant
                .try_get::<bool>()
                .map(TypedValue::Bool)
                .or_else(|_| variant.try_get::<i32>().map(TypedValue::Int))
                .or_else(|_| variant.try_get::<u32>().map(TypedValue::Uint))
                .or_else(|_| variant.try_get::<f64>().map(TypedValue::Double))
                .or_else(|_| {
                    variant
                        .try_get::<String>()
                        .map(Cow::Owned)
                        .map(TypedValue::String)
                })
                .or_else(|_| {
                    variant
                        .try_get::<Vec<glib::Variant>>()
                        .map_err(Error::from)
                        .and_then(|array| {
                            array
                                .into_iter()
                                .map(array_value_from_variant)
                                .map(|value| {
                                    value.map(|value| Value {
                                        value,
                                        props: Properties::default(),
                                    })
                                })
                                .collect::<Result<Vec<_>>>()
                        })
                        .map(TypedValue::Array)
                })
                .with_context(|| {
                    format!("unknown value type {}", variant.type_().to_str())
                })
        }

        fn array_value_from_variant(
            variant: glib::Variant,
        ) -> Result<TypedValue<'static>> {
            variant
                .try_get::<bool>()
                .map(TypedValue::Bool)
                .or_else(|_| variant.try_get::<i32>().map(TypedValue::Int))
                .or_else(|_| variant.try_get::<u32>().map(TypedValue::Uint))
                .or_else(|_| variant.try_get::<f64>().map(TypedValue::Double))
                .or_else(|_| {
                    variant
                        .try_get::<String>()
                        .map(Cow::Owned)
                        .map(TypedValue::String)
                })
                .with_context(|| {
                    format!(
                        "unknown array value type {}",
                        variant.type_().to_str()
                    )
                })
        }

        channels
            .array_iter_str()
            .context("error reading iterating channels")?
            .map(|name| {
                let name = Cow::<'_, str>::Owned(name.to_string());
                let flattened_props = dbus
                    .call("GetAllProperties", (name.as_ref(), "/"))?
                    .try_child_value(0)
                    .context("GetAllProperties had empty return value")?
                    .iter()
                    .map(|prop| {
                        let (path, value) =
                            prop.try_get::<(String, glib::Variant)>()?;
                        let value = value_from_variant(value)?;
                        Ok((path, value))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let mut props = Properties::default();
                for (path, value) in flattened_props {
                    let path_len = path.matches('/').count();
                    assert!(path_len > 0);
                    // path starts with / so skip first empty element
                    let mut path_parts = path
                        .split('/')
                        .skip(1)
                        .map(|path_part| Cow::Owned(path_part.to_owned()));
                    // traverse prop tree for all but last path part
                    let props = path_parts.by_ref().take(path_len - 1).fold(
                        &mut props,
                        |props, path_part| {
                            &mut props
                                .0
                                .entry(path_part)
                                .or_insert_with(|| Value {
                                    value: TypedValue::Empty,
                                    props: Properties::default(),
                                })
                                .props
                        },
                    );
                    // insert the value using the last part (the prop name)
                    let name = path_parts.next().unwrap();
                    match props.0.entry(name) {
                        btree_map::Entry::Vacant(entry) => {
                            entry.insert(Value {
                                value,
                                props: Properties::default(),
                            });
                        },
                        btree_map::Entry::Occupied(entry) => {
                            entry.into_mut().value = value;
                        },
                    }
                }
                let channel = Channel {
                    name: name.clone(),
                    props,
                };
                Ok((name, channel))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(IdMap)
            .map(Self)
    }
}

impl<'a> crate::serde::Id for Channel<'a> {
    type Id = Cow<'a, str>;

    fn id(&self) -> &Self::Id {
        &self.name
    }
}

impl ClearPath<'static> {
    pub fn parse(input: &str) -> Result<Self> {
        let mut input_parts = input.split('.').peekable();
        let channel = input_parts.next().context("missing channel")?;
        let mut parts = Vec::new();
        let mut props = None;
        while let Some(part) = input_parts.next() {
            if input_parts.peek().is_none() {
                let (part, value_changed) =
                    if let Some(part) = part.strip_prefix('~') {
                        (part, true)
                    } else {
                        (part, false)
                    };
                let prefix = if let Some(prefix) = part.strip_suffix('*') {
                    (!prefix.is_empty()).then(|| prefix)
                } else {
                    bail!("missing `*` in final prop specifier")
                };
                props = Some(ClearPathProps {
                    value_changed,
                    prefix: prefix.map(|prefix| prefix.to_owned().into()),
                });
            } else {
                let (prop, prefix) = if let Some(prop) = part.strip_suffix('*')
                {
                    (prop, true)
                } else {
                    (part, false)
                };
                parts.push(ClearPathPart {
                    prop: prop.to_owned().into(),
                    prefix,
                });
            }
        }
        if parts.is_empty() {
            bail!("missing root prop");
        }
        let props = props.context("missing final prop specifier")?;
        Ok(Self {
            channel: channel.to_owned().into(),
            parts,
            props,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct XfconfPatch<'a> {
    #[serde(skip_serializing_if = "ChannelsPatch::is_empty")]
    channels: ChannelsPatch<'a>,
}

impl<'a> XfconfPatch<'a> {
    pub fn diff(
        old: Xfconf<'a>,
        new: Xfconf<'a>,
        clear_paths: &[ClearPath<'_>],
    ) -> Self {
        Self {
            channels: ChannelsPatch::diff(
                old.channels,
                new.channels,
                clear_paths,
            ),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    pub fn has_panel_changes(&self) -> bool {
        self.channels.changed.contains_key("xfce4-panel")
            || self
                .channels
                .added
                .iter()
                .any(|channel| channel.name == "xfce4-panel")
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ChannelsPatch<'a> {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    changed: BTreeMap<Cow<'a, str>, ChannelPatch<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    added: Vec<Channel<'a>>,
}

impl<'a> ChannelsPatch<'a> {
    fn diff(
        mut old: Channels<'a>,
        new: Channels<'a>,
        clear_paths: &[ClearPath<'_>],
    ) -> Self {
        let mut changed = BTreeMap::new();
        let mut added = Vec::new();
        for (key, new_value) in (new.0).0.into_iter() {
            if let Some(old_value) = (old.0).0.remove(&key) {
                let patch =
                    ChannelPatch::diff(old_value, new_value, clear_paths);
                if !patch.is_empty() {
                    changed.insert(key, patch);
                }
            } else {
                added.push(new_value);
            }
        }
        Self { changed, added }
    }

    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ChannelPatch<'a> {
    #[serde(skip_serializing_if = "SimplePatch::is_empty")]
    name: SimplePatch<Cow<'a, str>>,
    #[serde(skip_serializing_if = "PropertiesPatch::is_empty")]
    props: PropertiesPatch<'a>,
}

impl<'a> ChannelPatch<'a> {
    fn diff(
        old: Channel<'a>,
        new: Channel<'a>,
        clear_paths: &[ClearPath<'_>],
    ) -> Self {
        let path = DiffPath {
            channel: None,
            props: im::Vector::new(),
        };
        let properties_ctx = PropertiesCtx::Channel(old.clone(), new.clone());
        Self {
            name: SimplePatch::diff(old.name, new.name),
            props: PropertiesPatch::diff(
                old.props,
                new.props,
                &path,
                properties_ctx,
                clear_paths,
            ),
        }
    }

    fn is_empty(&self) -> bool {
        self.name.is_empty() && self.props.is_empty()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct PropertiesPatch<'a> {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    changed: BTreeMap<Cow<'a, str>, ValuePatch<'a>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    added: BTreeMap<Cow<'a, str>, Value<'a>>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    removed: BTreeSet<Cow<'a, str>>,
}

enum PropertiesCtx<'a> {
    Channel(Channel<'a>, Channel<'a>),
    Value(Value<'a>, Value<'a>),
}

impl<'a> ClearPath<'a> {
    fn get_remove_keys_filter(
        &'a self,
        path: &DiffPath<'_>,
        ctx: &PropertiesCtx<'_>,
    ) -> Option<Box<dyn Fn(&Cow<'_, str>) -> bool + 'a>> {
        let ((_, channel), prop) = path.channel.as_ref()?;
        if channel.name != self.channel {
            return None;
        }
        if path.props.len() + 1 != self.parts.len() {
            return None;
        }
        if !iter::once(prop)
            .chain(path.props.iter().map(|(_, prop)| prop))
            .zip(self.parts.iter())
            .all(|(prop, part)| {
                if part.prefix {
                    prop.starts_with(&*part.prop)
                } else {
                    prop == &part.prop
                }
            })
        {
            return None;
        }
        if self.props.value_changed {
            if let PropertiesCtx::Value(old_ctx, new_ctx) = ctx {
                if old_ctx.value == new_ctx.value {
                    return None;
                }
            }
        }
        if let Some(prefix) = &self.props.prefix {
            Some(Box::new(move |key| key.starts_with(prefix.as_ref())))
        } else {
            Some(Box::new(move |_key| true))
        }
    }
}

impl<'a> PropertiesPatch<'a> {
    fn diff(
        mut old: Properties<'a>,
        new: Properties<'a>,
        path: &DiffPath<'a>,
        ctx: PropertiesCtx<'a>,
        clear_paths: &[ClearPath<'_>],
    ) -> Self {
        let mut changed = BTreeMap::new();
        let mut added = BTreeMap::new();
        for (key, new_value) in new.0.into_iter() {
            if let Some(old_value) = old.0.remove(&key) {
                let path = match &ctx {
                    PropertiesCtx::Channel(old_ctx, new_ctx) => path
                        .with_channel((
                            (old_ctx.clone(), new_ctx.clone()),
                            key.clone(),
                        )),
                    PropertiesCtx::Value(old_ctx, new_ctx) => path.push((
                        (old_ctx.clone(), new_ctx.clone()),
                        key.clone(),
                    )),
                };
                let patch =
                    ValuePatch::diff(old_value, new_value, &path, clear_paths);
                if !patch.is_empty() {
                    changed.insert(key, patch);
                }
            } else {
                added.insert(key, new_value);
            }
        }
        let removed = clear_paths
            .iter()
            .find_map(|clear_path| {
                clear_path.get_remove_keys_filter(path, &ctx)
            })
            .map_or_else(BTreeSet::new, |remove_keys_filter| {
                old.0
                    .into_keys()
                    .filter(remove_keys_filter)
                    .collect::<BTreeSet<_>>()
            });
        Self {
            changed,
            added,
            removed,
        }
    }

    fn is_empty(&self) -> bool {
        self.changed.is_empty()
            && self.added.is_empty()
            && self.removed.is_empty()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ValuePatch<'a> {
    #[serde(skip_serializing_if = "TypedValuePatch::is_empty")]
    value: TypedValuePatch<'a>,
    #[serde(skip_serializing_if = "PropertiesPatch::is_empty")]
    props: PropertiesPatch<'a>,
}

impl<'a> ValuePatch<'a> {
    fn diff(
        old: Value<'a>,
        new: Value<'a>,
        path: &DiffPath<'a>,
        clear_paths: &[ClearPath<'_>],
    ) -> Self {
        let properties_ctx = PropertiesCtx::Value(old.clone(), new.clone());
        Self {
            value: TypedValuePatch::diff(old.value, new.value),
            props: PropertiesPatch::diff(
                old.props,
                new.props,
                path,
                properties_ctx,
                clear_paths,
            ),
        }
    }

    fn is_empty(&self) -> bool {
        self.value.is_empty() && self.props.is_empty()
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
enum TypedValuePatch<'a> {
    Bool(SimplePatch<bool>),
    Int(SimplePatch<i32>),
    Uint(SimplePatch<u32>),
    Double(SimplePatch<f64>),
    String(SimplePatch<Cow<'a, str>>),
    Array(SimplePatch<Vec<Value<'a>>>),
    Empty,
    Changed(TypedValue<'a>),
}

impl<'a> TypedValuePatch<'a> {
    fn diff(old: TypedValue<'a>, new: TypedValue<'a>) -> Self {
        match (old, new) {
            (TypedValue::Bool(old_bool), TypedValue::Bool(new_bool)) => {
                Self::Bool(SimplePatch::diff(old_bool, new_bool))
            },
            (TypedValue::Int(old_int), TypedValue::Int(new_int)) => {
                Self::Int(SimplePatch::diff(old_int, new_int))
            },
            (TypedValue::Uint(old_uint), TypedValue::Uint(new_uint)) => {
                Self::Uint(SimplePatch::diff(old_uint, new_uint))
            },
            (
                TypedValue::Double(old_double),
                TypedValue::Double(new_double),
            ) => Self::Double(SimplePatch::diff(old_double, new_double)),
            (
                TypedValue::String(old_string),
                TypedValue::String(new_string),
            ) => Self::String(SimplePatch::diff(old_string, new_string)),
            (TypedValue::Array(old_array), TypedValue::Array(new_array)) => {
                Self::Array(SimplePatch::diff(old_array, new_array))
            },
            (TypedValue::Empty, TypedValue::Empty) => Self::Empty,
            (_old, new) => Self::Changed(new),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Bool(patch) => patch.is_empty(),
            Self::Int(patch) => patch.is_empty(),
            Self::Uint(patch) => patch.is_empty(),
            Self::Double(patch) => patch.is_empty(),
            Self::String(patch) => patch.is_empty(),
            Self::Array(patch) => patch.is_empty(),
            Self::Empty => true,
            Self::Changed(_) => false,
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
            value: (old != new).then(|| new),
        }
    }

    fn is_empty(&self) -> bool {
        self.value.is_none()
    }
}

#[derive(Debug, Clone)]
struct DiffPath<'a> {
    channel: Option<((Channel<'a>, Channel<'a>), Cow<'a, str>)>,
    props: im::Vector<((Value<'a>, Value<'a>), Cow<'a, str>)>,
}

impl<'a> DiffPath<'a> {
    fn with_channel(
        &self,
        channel: ((Channel<'a>, Channel<'a>), Cow<'a, str>),
    ) -> Self {
        let mut path = self.clone();
        path.channel = Some(channel);
        path
    }

    fn push(&self, prop: ((Value<'a>, Value<'a>), Cow<'a, str>)) -> Self {
        let mut path = self.clone();
        path.props.push_back(prop);
        path
    }
}

pub struct Applier<'a> {
    dry_run: bool,
    patch_recorder: &'a mut PatchRecorder,
    dbus: DBus,
}

impl<'a> Applier<'a> {
    pub(crate) fn new(
        dry_run: bool,
        patch_recorder: &'a mut PatchRecorder,
    ) -> Result<Self> {
        let dbus = DBus::new("org.xfce.Xfconf", "/org/xfce/Xfconf")?;
        Ok(Self {
            dry_run,
            patch_recorder,
            dbus,
        })
    }

    fn path_to_channel_property<'p>(
        path: &'p ApplyPath<'_>,
    ) -> (&'p str, String) {
        (
            &*path.channel,
            path.props
                .iter()
                .map(|prop| format!("/{}", prop))
                .collect::<String>(),
        )
    }

    fn call(
        &mut self,
        method: &'static str,
        args: impl glib::variant::ToVariant,
    ) -> Result<()> {
        self.patch_recorder
            .log(&crate::PatchEvent::Channel(PatchEvent::XfconfCall {
                method,
                args: variant_to_json(args.to_variant())
                    .context("error converting xfconf call args to JSON")?,
            }))
            .context("error logging xfconf call")?;
        if !self.dry_run {
            self.dbus.call(method, args)?;
        }
        Ok(())
    }

    fn set(
        &mut self,
        path: &ApplyPath<'_>,
        value: glib::Variant,
    ) -> Result<()> {
        let (channel, property) = Self::path_to_channel_property(path);
        let recursive = true;
        if self
            .dbus
            .call("PropertyExists", (channel, property.as_str()))
            .context("error checking if property exists")?
            .try_get::<(bool,)>()
            .context("error checking PropertyExists return")?
            .0
        {
            self.call("ResetProperty", (channel, property.as_str(), recursive))
                .context("error resetting property before set")?;
        }
        self.call("SetProperty", (channel, property.as_str(), value))
    }

    fn set_bool(&mut self, path: &ApplyPath<'_>, b: bool) -> Result<()> {
        self.set(path, glib::variant::ToVariant::to_variant(&b))
    }

    fn set_int(&mut self, path: &ApplyPath<'_>, n: i32) -> Result<()> {
        self.set(path, glib::variant::ToVariant::to_variant(&n))
    }

    fn set_uint(&mut self, path: &ApplyPath<'_>, n: u32) -> Result<()> {
        self.set(path, glib::variant::ToVariant::to_variant(&n))
    }

    fn set_double(&mut self, path: &ApplyPath<'_>, f: f64) -> Result<()> {
        self.set(path, glib::variant::ToVariant::to_variant(&f))
    }

    fn set_string(
        &mut self,
        path: &ApplyPath<'_>,
        s: Cow<'_, str>,
    ) -> Result<()> {
        self.set(path, glib::variant::ToVariant::to_variant(&*s))
    }

    fn set_array(
        &mut self,
        path: &ApplyPath<'_>,
        array: Vec<Value<'_>>,
    ) -> Result<()> {
        self.set(
            path,
            glib::variant::ToVariant::to_variant(
                &array
                    .into_iter()
                    .map(|value| match value.value {
                        TypedValue::Bool(b) => {
                            Ok(glib::variant::ToVariant::to_variant(&b))
                        },
                        TypedValue::Int(n) => {
                            Ok(glib::variant::ToVariant::to_variant(&n))
                        },
                        TypedValue::Uint(n) => {
                            Ok(glib::variant::ToVariant::to_variant(&n))
                        },
                        TypedValue::Double(f) => {
                            Ok(glib::variant::ToVariant::to_variant(&f))
                        },
                        TypedValue::String(s) => {
                            Ok(glib::variant::ToVariant::to_variant(&*s))
                        },
                        TypedValue::Array(_) => {
                            Err(anyhow!("array value in array value"))
                        },
                        TypedValue::Empty => {
                            Err(anyhow!("empty value in array value"))
                        },
                    })
                    .collect::<Result<Vec<_>>>()?,
            ),
        )
    }

    fn remove(&mut self, path: &ApplyPath<'_>) -> Result<()> {
        let (channel, property) = Self::path_to_channel_property(path);
        let recursive = true;
        self.call("ResetProperty", (channel, property.as_str(), recursive))
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PatchEvent<'a> {
    #[serde(rename_all = "kebab-case")]
    XfconfCall {
        method: &'a str,
        args: serde_json::Value,
    },
}

impl XfconfPatch<'_> {
    pub fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        self.channels.apply(applier)?;
        Ok(())
    }
}

impl ChannelsPatch<'_> {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        for (name, channel_patch) in self.changed {
            channel_patch.apply(applier, name)?;
        }
        for channel in self.added {
            channel.apply(applier)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ApplyPath<'a> {
    channel: Cow<'a, str>,
    props: im::Vector<Cow<'a, str>>,
}

impl<'a> ApplyPath<'a> {
    fn push(&self, prop: Cow<'a, str>) -> Self {
        let mut path = self.clone();
        path.props.push_back(prop);
        path
    }
}

impl Channel<'_> {
    fn apply(self, applier: &mut Applier<'_>) -> Result<()> {
        let path = ApplyPath {
            channel: self.name,
            props: im::Vector::new(),
        };
        self.props.apply(applier, &path)?;
        Ok(())
    }
}

impl<'a> Properties<'a> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        path: &ApplyPath<'a>,
    ) -> Result<()> {
        for (name, value) in self.0 {
            let path = path.push(name);
            value.apply(applier, &path)?;
        }
        Ok(())
    }
}

impl<'a> Value<'a> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        path: &ApplyPath<'a>,
    ) -> Result<()> {
        self.value.apply(applier, path)?;
        self.props.apply(applier, path)?;
        Ok(())
    }
}

impl<'a> TypedValue<'a> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        path: &ApplyPath<'a>,
    ) -> Result<()> {
        match self {
            Self::Bool(value) => applier.set_bool(path, value),
            Self::Int(value) => applier.set_int(path, value),
            Self::Uint(value) => applier.set_uint(path, value),
            Self::Double(value) => applier.set_double(path, value),
            Self::String(value) => applier.set_string(path, value),
            Self::Array(value) => applier.set_array(path, value),
            Self::Empty => Ok(()),
        }
    }
}

impl<'a> ChannelPatch<'a> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        name: Cow<'a, str>,
    ) -> Result<()> {
        let path = ApplyPath {
            channel: name,
            props: im::Vector::new(),
        };
        self.props.apply(applier, &path)?;
        Ok(())
    }
}

impl<'a> PropertiesPatch<'a> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        path: &ApplyPath<'a>,
    ) -> Result<()> {
        // keys of changed, added, removed are disjoint so order doesn't matter
        for (name, value_patch) in self.changed {
            let path = path.push(name);
            value_patch.apply(applier, &path)?;
        }
        for (name, value) in self.added {
            let path = path.push(name);
            value.apply(applier, &path)?;
        }
        for name in self.removed {
            let path = path.push(name);
            applier.remove(&path)?;
        }
        Ok(())
    }
}

impl<'a> ValuePatch<'a> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        path: &ApplyPath<'a>,
    ) -> Result<()> {
        self.value.apply(applier, path)?;
        self.props.apply(applier, path)?;
        Ok(())
    }
}

impl<'a> TypedValuePatch<'a> {
    fn apply(
        self,
        applier: &mut Applier<'_>,
        path: &ApplyPath<'a>,
    ) -> Result<()> {
        match self {
            Self::Bool(value_patch) => value_patch.apply(applier, path),
            Self::Int(value_patch) => value_patch.apply(applier, path),
            Self::Uint(value_patch) => value_patch.apply(applier, path),
            Self::Double(value_patch) => value_patch.apply(applier, path),
            Self::String(value_patch) => value_patch.apply(applier, path),
            Self::Array(value_patch) => value_patch.apply(applier, path),
            Self::Empty => Ok(()),
            Self::Changed(value) => value.apply(applier, path),
        }
    }
}

macro_rules! impl_simple_patch_apply {
    ($ty:ty, $set:ident) => {
        impl SimplePatch<$ty> {
            fn apply(
                self,
                applier: &mut Applier<'_>,
                path: &ApplyPath<'_>,
            ) -> Result<()> {
                if let Some(value) = self.value {
                    applier.$set(path, value)
                } else {
                    Ok(())
                }
            }
        }
    };
}
impl_simple_patch_apply!(bool, set_bool);
impl_simple_patch_apply!(i32, set_int);
impl_simple_patch_apply!(u32, set_uint);
impl_simple_patch_apply!(f64, set_double);
impl_simple_patch_apply!(Cow<'_, str>, set_string);
impl_simple_patch_apply!(Vec<Value<'_>>, set_array);

fn variant_to_json(v: glib::Variant) -> Result<serde_json::Value> {
    match v.type_().to_str() {
        "v" => variant_to_json(v.as_variant().unwrap()),
        "b" => Ok(serde_json::Value::from(v.get::<bool>().unwrap())),
        "i" => Ok(serde_json::Value::from(v.get::<i32>().unwrap())),
        "u" => Ok(serde_json::Value::from(v.get::<u32>().unwrap())),
        "d" => Ok(serde_json::Value::from(v.get::<f64>().unwrap())),
        "s" => Ok(serde_json::Value::from(v.get::<String>().unwrap())),
        r#type if r#type.starts_with('a') || r#type.starts_with('(') => v
            .iter()
            .map(variant_to_json)
            .collect::<Result<Vec<_>>>()
            .map(Into::into),
        r#type => bail!("bad arg type {}", r#type),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::btreemap;

    #[test]
    fn deserialize() {
        let channel: Channel<'static> = serde_json::from_str(
            r#"
            {
                "name": "channel",
                "version": "1.0",
                "props": {
                    "foo": {
                        "type": "string",
                        "value": "bar",
                        "props": {
                            "baz": {
                                "type": "uint",
                                "value": 42
                            }
                        }
                    }
                }
            }
            "#,
        )
        .unwrap();
        assert_eq!(
            channel,
            Channel {
                name: "channel".into(),
                props: Properties(btreemap! {
                    "foo".into() => Value {
                        value: TypedValue::String("bar".into()),
                        props: Properties(btreemap! {
                            "baz".into() => Value {
                                value: TypedValue::Uint(42),
                                props: Default::default(),
                            },
                        }),
                    },
                }),
            }
        );
    }
}
