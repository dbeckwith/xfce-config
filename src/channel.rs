use anyhow::Result;
use std::{borrow::Cow, io::Write};

#[derive(Debug)]
pub struct Channel<'a> {
    pub name: Cow<'a, str>,
    pub version: Cow<'a, str>,
    pub props: Vec<Property<'a>>,
}

#[derive(Debug)]
pub struct Property<'a> {
    pub name: Cow<'a, str>,
    pub value: Value<'a>,
}

#[derive(Debug)]
pub struct Value<'a> {
    pub value: TypedValue<'a>,
    pub props: Vec<Property<'a>>,
}

#[derive(Debug)]
pub enum TypedValue<'a> {
    Bool(bool),
    Int(i32),
    Uint(u32),
    String(Cow<'a, str>),
    Array(Vec<Value<'a>>),
    Empty,
}

impl<'a> Channel<'a> {
    pub fn new(
        name: impl Into<Cow<'a, str>>,
        version: impl Into<Cow<'a, str>>,
        props: Vec<Property<'a>>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            props,
        }
    }
}

impl<'a> Property<'a> {
    pub fn new(name: impl Into<Cow<'a, str>>, value: Value<'a>) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

impl<'a> Value<'a> {
    pub fn new(value: TypedValue<'a>, props: Vec<Property<'a>>) -> Self {
        Self { value, props }
    }

    pub fn bool(b: bool) -> Self {
        Self {
            value: TypedValue::Bool(b),
            props: Vec::new(),
        }
    }

    pub fn int(n: i32) -> Self {
        Self {
            value: TypedValue::Int(n),
            props: Vec::new(),
        }
    }

    pub fn uint(n: u32) -> Self {
        Self {
            value: TypedValue::Uint(n),
            props: Vec::new(),
        }
    }

    pub fn string(s: Cow<'a, str>) -> Self {
        Self {
            value: TypedValue::String(s),
            props: Vec::new(),
        }
    }

    pub fn array(items: Vec<Value<'a>>) -> Self {
        Self {
            value: TypedValue::Array(items),
            props: Vec::new(),
        }
    }

    pub fn empty(props: Vec<Property<'a>>) -> Self {
        Self {
            value: TypedValue::Empty,
            props,
        }
    }
}

impl Channel<'_> {
    pub fn write_xml<W>(&self, writer: W) -> Result<()>
    where
        W: Write,
    {
        use quick_xml::{
            events::{attributes::Attribute, BytesDecl, BytesStart, Event},
            Writer,
        };

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
                    TypedValue::Bool(_) => b"bool" as &[u8],
                    TypedValue::Int(_) => b"int",
                    TypedValue::Uint(_) => b"uint",
                    TypedValue::String(_) => b"string",
                    TypedValue::Array(_) => b"array",
                    TypedValue::Empty => b"empty",
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
                TypedValue::Bool(_) => &[],
                TypedValue::Int(_) => &[],
                TypedValue::Uint(_) => &[],
                TypedValue::String(_) => &[],
                TypedValue::Array(items) => items.as_slice(),
                TypedValue::Empty => &[],
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
            props: &[Property<'_>],
            writer: &mut Writer<W>,
        ) -> Result<()>
        where
            W: Write,
        {
            for Property { name, value } in props {
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

    #[test]
    fn write_xml() {
        let mut buf = Vec::new();
        let channel = Channel {
            name: "panel".into(),
            version: "1.0".into(),
            props: vec![
                Property {
                    name: "foo".into(),
                    value: Value {
                        value: TypedValue::Array(vec![
                            Value {
                                value: TypedValue::Bool(true),
                                props: vec![],
                            },
                            Value {
                                value: TypedValue::Bool(false),
                                props: vec![],
                            },
                        ]),
                        props: vec![],
                    },
                },
                Property {
                    name: "bar".into(),
                    value: Value {
                        value: TypedValue::Empty,
                        props: vec![Property {
                            name: "baz".into(),
                            value: Value {
                                value: TypedValue::String("qux".into()),
                                props: vec![],
                            },
                        }],
                    },
                },
            ],
        };
        channel.write_xml(&mut buf).unwrap();
        let xml = String::from_utf8(buf).unwrap();
        assert_eq!(
            xml,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<channel name="panel" version="1.0">
    <property name="foo" type="array">
        <value type="bool" value="true"/>
        <value type="bool" value="false"/>
    </property>
    <property name="bar" type="empty">
        <property name="baz" type="string" value="qux"/>
    </property>
</channel>
"#
        );
    }
}
