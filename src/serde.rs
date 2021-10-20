use serde::{de, ser};
use std::{
    collections::BTreeMap,
    fmt,
    iter::{self, FromIterator},
    marker::PhantomData,
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
