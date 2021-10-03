use crate::serde::IdMap;
use anyhow::{anyhow, bail, Context, Result};
use quick_xml::{
    events::{attributes::Attribute, BytesDecl, BytesStart, Event},
    Reader,
    Writer,
};
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs,
    io,
    io::{BufRead, Write},
    path::Path,
};

#[derive(Debug, Deserialize)]
pub struct Channels<'a>(IdMap<Channel<'a>>);

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Channel<'a> {
    name: Cow<'a, str>,
    version: Cow<'a, str>,
    #[serde(default)]
    props: Properties<'a>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
struct Properties<'a>(BTreeMap<Cow<'a, str>, Value<'a>>);

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Value<'a> {
    #[serde(flatten)]
    value: TypedValue<'a>,
    #[serde(default)]
    props: Properties<'a>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
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

impl Channels<'_> {
    pub fn read(dir: &Path) -> Result<Self> {
        dir.read_dir()
            .context("error reading dir")?
            .map(|entry| {
                let entry = entry.context("error reading dir entry")?;
                let path = entry.path();
                let file = fs::File::open(path)
                    .context("error opening channel XML file")?;
                let reader = io::BufReader::new(file);
                let channel = Channel::read_xml(reader)
                    .context("error reading channel XML")?;
                Ok(channel)
            })
            .map(|channel| {
                channel.map(|channel| (channel.name.clone(), channel))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(IdMap)
            .map(Self)
    }
}

impl Channel<'_> {
    fn read_xml<R>(reader: R) -> Result<Self>
    where
        R: BufRead,
    {
        fn make_value<R>(
            reader: &mut Reader<R>,
            buf: &mut Vec<u8>,
            parent_tag: &[u8],
            for_tag: &[u8],
            r#type: Option<Cow<'static, str>>,
            value: Option<Cow<'static, str>>,
        ) -> Result<Value<'static>>
        where
            R: BufRead,
        {
            let (sub_values, sub_props) = read_props(reader, buf, for_tag)
                .with_context(|| {
                    format!("{} props", String::from_utf8_lossy(for_tag))
                })?;
            let mut sub_values = Some(sub_values);
            let value = match r#type.context("missing type attribute")?.as_ref()
            {
                "bool" => TypedValue::Bool(
                    value
                        .context("missing value attribute")?
                        .parse()
                        .context("parsing value attribute as bool")?,
                ),
                "int" => TypedValue::Int(
                    value
                        .context("missing value attribute")?
                        .parse()
                        .context("parsing value attribute as int")?,
                ),
                "uint" => TypedValue::Uint(
                    value
                        .context("missing value attribute")?
                        .parse()
                        .context("parsing value attribute as uint")?,
                ),
                "double" => TypedValue::Double(
                    value
                        .context("missing value attribute")?
                        .parse()
                        .context("parsing value attribute as double")?,
                ),
                "string" => TypedValue::String(
                    value.context("missing value attribute")?,
                ),
                "array" => TypedValue::Array(sub_values.take().unwrap()),
                "empty" => TypedValue::Empty,
                r#type => bail!("unexpected value type {}", r#type),
            };
            if let Some(sub_values) = sub_values {
                if !sub_values.is_empty() {
                    bail!(
                        "unexpected value tags under {} tag",
                        String::from_utf8_lossy(parent_tag)
                    );
                }
            }
            Ok(Value {
                value,
                props: sub_props,
            })
        }

        fn read_props<R>(
            reader: &mut Reader<R>,
            buf: &mut Vec<u8>,
            parent_tag: &[u8],
        ) -> Result<(Vec<Value<'static>>, Properties<'static>)>
        where
            R: BufRead,
        {
            let mut values = Vec::new();
            let mut props = Properties::default();
            loop {
                match reader.read_event(buf)? {
                    Event::Start(tag) => match tag.name() {
                        b"property" => {
                            let mut name = None::<Cow<'static, str>>;
                            let mut r#type = None::<Cow<'static, str>>;
                            let mut value = None::<Cow<'static, str>>;
                            for attribute in tag.attributes() {
                                let attribute = attribute?;
                                match attribute.key {
                                    b"name" => {
                                        name = Some(
                                            attribute
                                                .unescape_and_decode_value(
                                                    reader,
                                                )
                                                .context(
                                                    "decoding name attribute",
                                                )?
                                                .into(),
                                        );
                                    },
                                    b"type" => {
                                        r#type = Some(
                                            attribute
                                                .unescape_and_decode_value(
                                                    reader,
                                                )
                                                .context(
                                                    "decoding type attribute",
                                                )?
                                                .into(),
                                        );
                                    },
                                    b"value" => {
                                        value = Some(
                                            attribute
                                                .unescape_and_decode_value(
                                                    reader,
                                                )
                                                .context(
                                                    "decoding value attribute",
                                                )?
                                                .into(),
                                        );
                                    },
                                    key => bail!(
                                        "unexpected attribute {}",
                                        String::from_utf8_lossy(key)
                                    ),
                                }
                            }
                            let name =
                                name.context("missing name attribute")?;
                            let value = make_value(
                                reader,
                                buf,
                                parent_tag,
                                b"property",
                                r#type,
                                value,
                            )?;
                            if props.0.insert(name.clone(), value).is_some() {
                                bail!("duplicate property {}", name);
                            }
                        },
                        b"value" => {
                            let mut r#type = None::<Cow<'static, str>>;
                            let mut value = None::<Cow<'static, str>>;
                            for attribute in tag.attributes() {
                                let attribute = attribute?;
                                match attribute.key {
                                    b"type" => {
                                        r#type = Some(
                                            attribute
                                                .unescape_and_decode_value(
                                                    reader,
                                                )
                                                .context(
                                                    "decoding type attribute",
                                                )?
                                                .into(),
                                        );
                                    },
                                    b"value" => {
                                        value = Some(
                                            attribute
                                                .unescape_and_decode_value(
                                                    reader,
                                                )
                                                .context(
                                                    "decoding value attribute",
                                                )?
                                                .into(),
                                        );
                                    },
                                    key => bail!(
                                        "unexpected attribute {}",
                                        String::from_utf8_lossy(key)
                                    ),
                                }
                            }
                            let value = make_value(
                                reader, buf, parent_tag, b"value", r#type,
                                value,
                            )?;
                            values.push(value);
                        },
                        tag => bail!(
                            "unexpected tag {}",
                            String::from_utf8_lossy(tag)
                        ),
                    },
                    Event::End(tag) => {
                        if tag.name() == parent_tag {
                            break;
                        } else {
                            bail!(
                                "expected {} end",
                                String::from_utf8_lossy(parent_tag)
                            );
                        }
                    },
                    event => bail!("unexpected event {:?}", event),
                }
            }
            Ok((values, props))
        }

        fn read_channel<R>(
            reader: &mut Reader<R>,
            buf: &mut Vec<u8>,
        ) -> Result<Channel<'static>>
        where
            R: BufRead,
        {
            let tag = match reader.read_event(buf)? {
                Event::Start(tag) => tag,
                event => bail!("expected tag start, got {:?}", event),
            };
            if tag.name() != b"channel" {
                bail!("expected channel tag");
            }
            let mut name = None::<Cow<'static, str>>;
            let mut version = None::<Cow<'static, str>>;
            for attribute in tag.attributes() {
                let attribute = attribute?;
                match attribute.key {
                    b"name" => {
                        name = Some(
                            attribute
                                .unescape_and_decode_value(reader)
                                .context("decoding name attribute")?
                                .into(),
                        );
                    },
                    b"version" => {
                        version = Some(
                            attribute
                                .unescape_and_decode_value(reader)
                                .context("decoding version attribute")?
                                .into(),
                        );
                    },
                    key => bail!(
                        "unexpected attribute {}",
                        String::from_utf8_lossy(key)
                    ),
                }
            }
            let name = name.context("missing name attribute")?;
            let version = version.context("missing version attribute")?;
            let (values, props) =
                read_props(reader, buf, b"channel").context("channel props")?;
            if !values.is_empty() {
                bail!("unexpected value tags under channel tag");
            }
            Ok(Channel {
                name,
                version,
                props,
            })
        }

        let mut reader = Reader::from_reader(reader);
        reader.expand_empty_elements(true);
        reader.trim_text(true);
        let mut buf = Vec::new();
        let decl = match reader.read_event(&mut buf)? {
            Event::Decl(decl) => decl,
            event => bail!("expected decl, got {:?}", event),
        };
        let decl_version = decl.version()?;
        if decl_version.as_ref() != b"1.0" {
            bail!(
                "unexpected XML version {}",
                String::from_utf8_lossy(decl_version.as_ref())
            );
        }
        let decl_encoding = decl.encoding().context("missing encoding")??;
        if decl_encoding.as_ref() != b"UTF-8" {
            bail!(
                "unexpected XML encoding {}",
                String::from_utf8_lossy(decl_encoding.as_ref())
            );
        }
        read_channel(&mut reader, &mut buf)
    }

    #[allow(dead_code)]
    fn write_xml<W>(&self, writer: W) -> Result<()>
    where
        W: Write,
    {
        fn write_value<W>(
            value: &Value<'_>,
            tag: Option<BytesStart<'static>>,
            writer: &mut Writer<W>,
        ) -> Result<()>
        where
            W: Write,
        {
            let Value { value, props } = value;

            let mut tag =
                tag.unwrap_or_else(|| BytesStart::borrowed_name(b"value"));

            tag.push_attribute(Attribute {
                key: b"type",
                value: match value {
                    TypedValue::Bool { .. } => b"bool" as &[u8],
                    TypedValue::Int { .. } => b"int",
                    TypedValue::Uint { .. } => b"uint",
                    TypedValue::Double { .. } => b"double",
                    TypedValue::String { .. } => b"string",
                    TypedValue::Array { .. } => b"array",
                    TypedValue::Empty { .. } => b"empty",
                }
                .into(),
            });

            match value {
                TypedValue::Bool(b) => {
                    tag.push_attribute(Attribute {
                        key: b"value",
                        value: if *b { b"true" as &[u8] } else { b"false" }
                            .into(),
                    });
                },
                TypedValue::Int(n) => {
                    tag.push_attribute(Attribute {
                        key: b"value",
                        value: n.to_string().into_bytes().into(),
                    });
                },
                TypedValue::Uint(n) => {
                    tag.push_attribute(Attribute {
                        key: b"value",
                        value: n.to_string().into_bytes().into(),
                    });
                },
                TypedValue::Double(f) => {
                    tag.push_attribute(Attribute {
                        key: b"value",
                        value: f.to_string().into_bytes().into(),
                    });
                },
                TypedValue::String(s) => {
                    tag.push_attribute(Attribute {
                        key: b"value",
                        value: s.as_bytes().into(),
                    });
                },
                TypedValue::Array(_array) => {},
                TypedValue::Empty => {},
            }

            let sub_values = match value {
                TypedValue::Array(array) => array.as_slice(),
                _ => &[],
            };

            if props.0.is_empty() && sub_values.is_empty() {
                writer.write_event(Event::Empty(tag))?;
            } else {
                let end = tag.to_end();
                writer.write_event(Event::Start(tag.to_borrowed()))?;
                for sub_value in sub_values {
                    write_value(sub_value, None, writer)?;
                }
                write_props(props, writer)?;
                writer.write_event(Event::End(end))?;
            }

            Ok(())
        }

        fn write_props<W>(
            props: &Properties<'_>,
            writer: &mut Writer<W>,
        ) -> Result<()>
        where
            W: Write,
        {
            for (name, value) in &props.0 {
                let mut tag = BytesStart::borrowed_name(b"property");
                tag.push_attribute(Attribute {
                    key: b"name",
                    value: name.as_bytes().into(),
                });
                write_value(value, Some(tag), writer)?;
            }
            Ok(())
        }

        let mut writer = Writer::new_with_indent(writer, b' ', 4);

        writer.write_event(Event::Decl(BytesDecl::new(
            b"1.0",
            Some(b"UTF-8"),
            None,
        )))?;

        let Self {
            name,
            version,
            props,
        } = self;

        let mut tag = BytesStart::borrowed_name(b"channel");

        tag.push_attribute(Attribute {
            key: b"name",
            value: name.as_bytes().into(),
        });

        tag.push_attribute(Attribute {
            key: b"version",
            value: version.as_bytes().into(),
        });

        if props.0.is_empty() {
            writer.write_event(Event::Empty(tag))?;
        } else {
            let end = tag.to_end();
            writer.write_event(Event::Start(tag.to_borrowed()))?;
            write_props(props, &mut writer)?;
            writer.write_event(Event::End(end))?;
        }

        writeln!(writer.inner())?;

        Ok(())
    }
}

impl<'a> crate::serde::Id for Channel<'a> {
    type Id = Cow<'a, str>;

    fn id(&self) -> &Self::Id {
        &self.name
    }
}

#[derive(Debug)]
pub struct ChannelsPatch<'a> {
    changed: BTreeMap<Cow<'a, str>, ChannelPatch<'a>>,
    added: Vec<Channel<'a>>,
}

impl<'a> ChannelsPatch<'a> {
    pub fn diff(mut old: Channels<'a>, new: Channels<'a>) -> Self {
        let mut changed = BTreeMap::new();
        let mut added = Vec::new();
        for (key, new_value) in (new.0).0.into_iter() {
            if let Some(old_value) = (old.0).0.remove(&key) {
                let patch = ChannelPatch::diff(old_value, new_value);
                if !patch.is_empty() {
                    changed.insert(key, patch);
                }
            } else {
                added.push(new_value);
            }
        }
        Self { changed, added }
    }

    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty()
    }
}

#[derive(Debug)]
struct ChannelPatch<'a> {
    name: SimplePatch<Cow<'a, str>>,
    version: SimplePatch<Cow<'a, str>>,
    props: PropertiesPatch<'a>,
}

impl<'a> ChannelPatch<'a> {
    fn diff(old: Channel<'a>, new: Channel<'a>) -> Self {
        let path = DiffPath {
            channel: None,
            props: im::Vector::new(),
        };
        let properties_ctx = PropertiesCtx::Channel(old.clone(), new.clone());
        Self {
            name: SimplePatch::diff(old.name, new.name),
            version: SimplePatch::diff(old.version, new.version),
            props: PropertiesPatch::diff(
                old.props,
                new.props,
                &path,
                properties_ctx,
            ),
        }
    }

    fn is_empty(&self) -> bool {
        self.name.is_empty() && self.version.is_empty() && self.props.is_empty()
    }
}

#[derive(Debug)]
struct PropertiesPatch<'a> {
    changed: BTreeMap<Cow<'a, str>, ValuePatch<'a>>,
    added: BTreeMap<Cow<'a, str>, Value<'a>>,
    removed: BTreeSet<Cow<'a, str>>,
}

enum PropertiesCtx<'a> {
    Channel(Channel<'a>, Channel<'a>),
    Value(Value<'a>, Value<'a>),
}

impl<'a> PropertiesPatch<'a> {
    fn diff(
        mut old: Properties<'a>,
        new: Properties<'a>,
        path: &DiffPath<'a>,
        ctx: PropertiesCtx<'a>,
    ) -> Self {
        let remove_old = (|| {
            use if_chain::if_chain;
            // remove old panels
            if_chain! {
                if let Some(((_, channel), prop)) = path.channel.as_ref();
                if channel.name == "xfce4-panel";
                if prop == "panels";
                let mut path_props = path.props.iter();
                if path_props.next().is_none();
                then { return true; }
            }
            // remove old plugins
            if_chain! {
                if let Some(((_, channel), prop)) = path.channel.as_ref();
                if channel.name == "xfce4-panel";
                if prop == "plugins";
                let mut path_props = path.props.iter();
                if path_props.next().is_none();
                then { return true; }
            }
            // remove old props when plugin type changes
            if_chain! {
                if let Some(((_, channel), prop)) = path.channel.as_ref();
                if channel.name == "xfce4-panel";
                if prop == "plugins";
                let mut path_props = path.props.iter();
                if let Some((_, _)) = path_props.next();
                if path_props.next().is_none();
                if let PropertiesCtx::Value(old_ctx, new_ctx) = &ctx;
                if old_ctx.value != new_ctx.value;
                then { return true; }
            }
            false
        })();
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
                let patch = ValuePatch::diff(old_value, new_value, &path);
                if !patch.is_empty() {
                    changed.insert(key, patch);
                }
            } else {
                added.insert(key, new_value);
            }
        }
        let removed = if remove_old {
            old.0.into_keys().collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
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

#[derive(Debug)]
struct ValuePatch<'a> {
    value: TypedValuePatch<'a>,
    props: PropertiesPatch<'a>,
}

impl<'a> ValuePatch<'a> {
    fn diff(old: Value<'a>, new: Value<'a>, path: &DiffPath<'a>) -> Self {
        let properties_ctx = PropertiesCtx::Value(old.clone(), new.clone());
        Self {
            value: TypedValuePatch::diff(old.value, new.value),
            props: PropertiesPatch::diff(
                old.props,
                new.props,
                path,
                properties_ctx,
            ),
        }
    }

    fn is_empty(&self) -> bool {
        self.value.is_empty() && self.props.is_empty()
    }
}

#[derive(Debug)]
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
            Self::Changed(_) => true,
        }
    }
}

#[derive(Debug)]
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

pub struct Applier {
    dry_run: bool,
    dbus: gio::DBusProxy,
}

impl Applier {
    #[allow(clippy::new_without_default)]
    pub fn new(dry_run: bool) -> Result<Self> {
        let dbus = gio::DBusProxy::for_bus_sync::<gio::Cancellable>(
            gio::BusType::Session,
            gio::DBusProxyFlags::NONE,
            None,
            "org.xfce.Xfconf",
            "/org/xfce/Xfconf",
            "org.xfce.Xfconf",
            None,
        )
        .context("error creating dbus proxy")?;
        Ok(Self { dry_run, dbus })
    }

    fn path_to_channel_property<'a>(
        path: &'a ApplyPath<'_>,
    ) -> (&'a str, String) {
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
        args: impl glib::variant::ToVariant + fmt::Debug,
    ) -> Result<()> {
        let args = args.to_variant();
        eprintln!("call {}{}", method, args.to_string());
        if !self.dry_run {
            gio::prelude::DBusProxyExt::call_sync::<gio::Cancellable>(
                &self.dbus,
                method,
                Some(&args),
                gio::DBusCallFlags::NONE,
                -1,
                None,
            )
            .with_context(|| format!("{}{}", method, args.to_string()))?;
        }
        Ok(())
    }

    fn set(
        &mut self,
        path: &ApplyPath<'_>,
        value: glib::Variant,
    ) -> Result<()> {
        let (channel, property) = Self::path_to_channel_property(path);
        self.call("SetProperty", &(channel, property, value))
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
        self.call("ResetProperty", &(channel, property, true))
    }
}

impl ChannelsPatch<'_> {
    pub fn apply(self, applier: &mut Applier) -> Result<()> {
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
    fn apply(self, applier: &mut Applier) -> Result<()> {
        let path = ApplyPath {
            channel: self.name,
            props: im::Vector::new(),
        };
        self.props.apply(applier, &path)?;
        Ok(())
    }
}

impl<'a> Properties<'a> {
    fn apply(self, applier: &mut Applier, path: &ApplyPath<'a>) -> Result<()> {
        for (name, value) in self.0 {
            let path = path.push(name);
            value.apply(applier, &path)?;
        }
        Ok(())
    }
}

impl<'a> Value<'a> {
    fn apply(self, applier: &mut Applier, path: &ApplyPath<'a>) -> Result<()> {
        self.value.apply(applier, path)?;
        self.props.apply(applier, path)?;
        Ok(())
    }
}

impl<'a> TypedValue<'a> {
    fn apply(self, applier: &mut Applier, path: &ApplyPath<'a>) -> Result<()> {
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
    fn apply(self, applier: &mut Applier, name: Cow<'a, str>) -> Result<()> {
        let path = ApplyPath {
            channel: name,
            props: im::Vector::new(),
        };
        self.props.apply(applier, &path)?;
        Ok(())
    }
}

impl<'a> PropertiesPatch<'a> {
    fn apply(self, applier: &mut Applier, path: &ApplyPath<'a>) -> Result<()> {
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
    fn apply(self, applier: &mut Applier, path: &ApplyPath<'a>) -> Result<()> {
        self.value.apply(applier, path)?;
        self.props.apply(applier, path)?;
        Ok(())
    }
}

impl<'a> TypedValuePatch<'a> {
    fn apply(self, applier: &mut Applier, path: &ApplyPath<'a>) -> Result<()> {
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
                applier: &mut Applier,
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

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::btreemap;

    #[test]
    fn read_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<channel name="panel" version="1.0">
    <property name="bar" type="empty">
        <property name="baz" type="string" value="qux"/>
    </property>
    <property name="foo" type="array">
        <value type="bool" value="true"/>
        <value type="bool" value="false"/>
    </property>
</channel>
"#;
        let channel = Channel::read_xml(xml.as_bytes()).unwrap();
        assert_eq!(
            channel,
            Channel {
                name: "panel".into(),
                version: "1.0".into(),
                props: Properties(btreemap! {
                    "foo".into() => Value {
                        value: TypedValue::Array(vec![
                            Value {
                                value: TypedValue::Bool(true),
                                props: Default::default(),
                            },
                            Value {
                                value: TypedValue::Bool(false),
                                props: Default::default(),
                            },
                        ]),
                        props: Default::default(),
                    },
                    "bar".into() => Value {
                        value: TypedValue::Empty,
                        props: Properties(btreemap! {
                            "baz".into() => Value {
                                value: TypedValue::String("qux".into()),
                                props: Default::default(),
                            },
                        }),
                    },
                }),
            }
        );
    }

    #[test]
    fn write_xml() {
        let mut buf = Vec::new();
        let channel = Channel {
            name: "panel".into(),
            version: "1.0".into(),
            props: Properties(btreemap! {
                "foo".into() => Value {
                    value: TypedValue::Array(vec![
                        Value {
                            value: TypedValue::Bool(true),
                            props: Default::default(),
                        },
                        Value {
                            value: TypedValue::Bool(false),
                            props: Default::default(),
                        },
                    ]),
                    props: Default::default(),
                },
                "bar".into() => Value {
                    value: TypedValue::Empty,
                    props: Properties(btreemap! {
                        "baz".into() => Value {
                            value: TypedValue::String("qux".into()),
                            props: Default::default(),
                        },
                    }),
                },
            }),
        };
        channel.write_xml(&mut buf).unwrap();
        let xml = String::from_utf8(buf).unwrap();
        assert_eq!(
            xml,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<channel name="panel" version="1.0">
    <property name="bar" type="empty">
        <property name="baz" type="string" value="qux"/>
    </property>
    <property name="foo" type="array">
        <value type="bool" value="true"/>
        <value type="bool" value="false"/>
    </property>
</channel>
"#
        );
    }

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
                version: "1.0".into(),
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
