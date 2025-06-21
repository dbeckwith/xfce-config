use serde::{de, ser};
use std::{
    collections::BTreeMap,
    fmt,
    iter::{self, FromIterator},
    marker::PhantomData,
    ops::Deref,
    path::{Path, PathBuf},
};

pub trait Id {
    type Id: Clone + Ord;

    fn id(&self) -> &Self::Id;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdMap<T>(pub BTreeMap<T::Id, T>)
where
    T: Id;

impl<T> Default for IdMap<T>
where
    T: Id,
{
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<T> IdMap<T>
where
    T: Id,
{
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> FromIterator<T> for IdMap<T>
where
    T: Id,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|item| (item.id().clone(), item))
                .collect::<BTreeMap<_, _>>(),
        )
    }
}

impl<T> ser::Serialize for IdMap<T>
where
    T: ser::Serialize + Id,
    T::Id: ser::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.collect_seq(self.0.values())
    }
}

impl<'de, T> de::Deserialize<'de> for IdMap<T>
where
    T: de::Deserialize<'de> + Id,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<T>(PhantomData<T>);

        impl<'de, T> de::Visitor<'de> for Visitor<T>
        where
            T: de::Deserialize<'de> + Id,
        {
            type Value = IdMap<T>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "id mapped list")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                iter::from_fn(|| seq.next_element().transpose()).collect()
            }
        }

        deserializer.deserialize_seq(Visitor(PhantomData))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RelativePathBuf(PathBuf);

#[derive(Debug)]
pub struct RelativePathBufError {
    _priv: (),
}

impl fmt::Display for RelativePathBufError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "path is not relative")
    }
}

impl std::error::Error for RelativePathBufError {}

impl RelativePathBuf {
    pub fn new(path: PathBuf) -> Result<Self, RelativePathBufError> {
        if path.is_relative() {
            Ok(Self(path))
        } else {
            Err(RelativePathBufError { _priv: () })
        }
    }
}

impl Deref for RelativePathBuf {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<PathBuf> for RelativePathBuf {
    fn as_ref(&self) -> &PathBuf {
        &self.0
    }
}

impl AsRef<Path> for RelativePathBuf {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl ser::Serialize for RelativePathBuf {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> de::Deserialize<'de> for RelativePathBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = RelativePathBuf;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "relative path")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                RelativePathBuf::new(Path::new(v).to_owned()).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}
