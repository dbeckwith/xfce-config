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
