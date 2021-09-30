use crate::diff;
use anyhow::{bail, Context, Result};
use quick_xml::{
    events::{attributes::Attribute, BytesDecl, BytesStart, Event},
    Reader,
    Writer,
};
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::BTreeMap,
    io::{BufRead, Write},
};

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Channel<'a> {
    pub name: Cow<'a, str>,
    pub version: Cow<'a, str>,
    #[serde(default)]
    pub props: Properties<'a>,
}

pub type Properties<'a> = BTreeMap<Cow<'a, str>, Value<'a>>;

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Value<'a> {
    #[serde(flatten)]
    pub value: TypedValue<'a>,
    #[serde(default)]
    pub props: Properties<'a>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
#[cfg_attr(test, derive(PartialEq))]
pub enum TypedValue<'a> {
    Bool(bool),
    Int(i32),
    Uint(u32),
    Double(f64),
    String(Cow<'a, str>),
    Array(Vec<Value<'a>>),
    Empty,
}

impl<'a> Channel<'a> {
    pub fn new(
        name: impl Into<Cow<'a, str>>,
        version: impl Into<Cow<'a, str>>,
        props: Properties<'a>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            props,
        }
    }
}

impl<'a> Value<'a> {
    pub fn new(value: TypedValue<'a>, props: Properties<'a>) -> Self {
        Self { value, props }
    }

    pub fn bool(b: bool) -> Self {
        Self {
            value: TypedValue::Bool(b),
            props: Default::default(),
        }
    }

    pub fn int(n: i32) -> Self {
        Self {
            value: TypedValue::Int(n),
            props: Default::default(),
        }
    }

    pub fn uint(n: u32) -> Self {
        Self {
            value: TypedValue::Uint(n),
            props: Default::default(),
        }
    }

    pub fn double(f: f64) -> Self {
        Self {
            value: TypedValue::Double(f),
            props: Default::default(),
        }
    }

    pub fn string(s: impl Into<Cow<'a, str>>) -> Self {
        Self {
            value: TypedValue::String(s.into()),
            props: Default::default(),
        }
    }

    pub fn array(items: Vec<Value<'a>>) -> Self {
        Self {
            value: TypedValue::Array(items),
            props: Default::default(),
        }
    }

    pub fn empty(props: Properties<'a>) -> Self {
        Self {
            value: TypedValue::Empty,
            props,
        }
    }
}

#[derive(Debug)]
pub struct ChannelPatch<'a> {
    name: <Cow<'a, str> as diff::Diff>::Patch,
    version: <Cow<'a, str> as diff::Diff>::Patch,
    props: <Properties<'a> as diff::Diff>::Patch,
}

impl<'a> diff::Diff for Channel<'a> {
    type Patch = ChannelPatch<'a>;

    fn diff(&self, other: &Self) -> Self::Patch {
        ChannelPatch {
            name: self.name.diff(&other.name),
            version: self.version.diff(&other.version),
            props: self.props.diff(&other.props),
        }
    }
}

impl diff::Patch for ChannelPatch<'_> {
    fn is_empty(&self) -> bool {
        self.name.is_empty() && self.version.is_empty() && self.props.is_empty()
    }
}

#[derive(Debug)]
pub struct ValuePatch<'a> {
    value: <TypedValue<'a> as diff::Diff>::Patch,
    props: <Properties<'a> as diff::Diff>::Patch,
}

impl<'a> diff::Diff for Value<'a> {
    type Patch = ValuePatch<'a>;

    fn diff(&self, other: &Self) -> Self::Patch {
        ValuePatch {
            value: self.value.diff(&other.value),
            props: self.props.diff(&other.props),
        }
    }
}

impl diff::Patch for ValuePatch<'_> {
    fn is_empty(&self) -> bool {
        self.value.is_empty() && self.props.is_empty()
    }
}

#[derive(Debug)]
pub enum TypedValuePatch<'a> {
    Bool(<bool as diff::Diff>::Patch),
    Int(<i32 as diff::Diff>::Patch),
    Uint(<u32 as diff::Diff>::Patch),
    Double(<f64 as diff::Diff>::Patch),
    String(<Cow<'a, str> as diff::Diff>::Patch),
    Array(<Vec<Value<'a>> as diff::Diff>::Patch),
    Empty,
    Changed(TypedValue<'a>),
}

impl<'a> diff::Diff for TypedValue<'a> {
    type Patch = TypedValuePatch<'a>;

    fn diff(&self, other: &Self) -> Self::Patch {
        match (self, other) {
            (Self::Bool(self_bool), Self::Bool(other_bool)) => {
                TypedValuePatch::Bool(self_bool.diff(other_bool))
            },
            (Self::Int(self_int), Self::Int(other_int)) => {
                TypedValuePatch::Int(self_int.diff(other_int))
            },
            (Self::Uint(self_uint), Self::Uint(other_uint)) => {
                TypedValuePatch::Uint(self_uint.diff(other_uint))
            },
            (Self::Double(self_double), Self::Double(other_double)) => {
                TypedValuePatch::Double(self_double.diff(other_double))
            },
            (Self::String(self_string), Self::String(other_string)) => {
                TypedValuePatch::String(self_string.diff(other_string))
            },
            (Self::Array(self_array), Self::Array(other_array)) => {
                TypedValuePatch::Array(self_array.diff(other_array))
            },
            (Self::Empty, Self::Empty) => TypedValuePatch::Empty,
            (_self, other) => TypedValuePatch::Changed(other.clone()),
        }
    }
}

impl diff::Patch for TypedValuePatch<'_> {
    fn is_empty(&self) -> bool {
        match self {
            TypedValuePatch::Bool(patch) => patch.is_empty(),
            TypedValuePatch::Int(patch) => patch.is_empty(),
            TypedValuePatch::Uint(patch) => patch.is_empty(),
            TypedValuePatch::Double(patch) => patch.is_empty(),
            TypedValuePatch::String(patch) => patch.is_empty(),
            TypedValuePatch::Array(patch) => patch.is_empty(),
            TypedValuePatch::Empty => true,
            TypedValuePatch::Changed(_) => true,
        }
    }
}

impl Channel<'_> {
    pub fn read_xml<R>(reader: R) -> Result<Self>
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
            let mut props = Properties::new();
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
                            if props.insert(name.clone(), value).is_some() {
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

    pub fn write_xml<W>(&self, writer: W) -> Result<()>
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
                TypedValue::Array(_items) => {},
                TypedValue::Empty => {},
            }

            let sub_values = match value {
                TypedValue::Array(items) => items.as_slice(),
                _ => &[],
            };

            if props.is_empty() && sub_values.is_empty() {
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
            for (name, value) in props {
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

        if props.is_empty() {
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
                props: btreemap! {
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
                        props: btreemap! {
                            "baz".into() => Value {
                                value: TypedValue::String("qux".into()),
                                props: Default::default(),
                            },
                        },
                    },
                },
            }
        );
    }

    #[test]
    fn write_xml() {
        let mut buf = Vec::new();
        let channel = Channel {
            name: "panel".into(),
            version: "1.0".into(),
            props: btreemap! {
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
                    props: btreemap! {
                        "baz".into() => Value {
                            value: TypedValue::String("qux".into()),
                            props: Default::default(),
                        },
                    },
                },
            },
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
                props: btreemap! {
                    "foo".into() => Value {
                        value: TypedValue::String("bar".into()),
                        props: btreemap! {
                            "baz".into() => Value {
                                value: TypedValue::Uint(42),
                                props: Default::default(),
                            },
                        },
                    },
                },
            }
        );
    }
}
