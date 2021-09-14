use anyhow::Result;
use std::io::Write;

#[derive(Debug)]
pub struct Channel {
    pub name: String,
    pub version: String,
    pub props: Vec<Property>,
}

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub value: Value,
}

#[derive(Debug)]
pub struct Value {
    pub value: TypedValue,
    pub props: Vec<Property>,
}

#[derive(Debug)]
pub enum TypedValue {
    Bool(bool),
    Int(i32),
    Uint(u32),
    String(String),
    Array(Vec<Value>),
    Empty,
}

impl Channel {
    pub fn write_xml<W>(&self, writer: W) -> Result<()>
    where
        W: Write,
    {
        use quick_xml::{
            events::{attributes::Attribute, BytesDecl, BytesStart, Event},
            Writer,
        };

        fn write_value<W>(
            value: &Value,
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
            props: &[Property],
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
            name: "panel".to_owned(),
            version: "1.0".to_owned(),
            props: vec![
                Property {
                    name: "foo".to_owned(),
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
                    name: "bar".to_owned(),
                    value: Value {
                        value: TypedValue::Empty,
                        props: vec![Property {
                            name: "baz".to_owned(),
                            value: Value {
                                value: TypedValue::String("qux".to_owned()),
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
