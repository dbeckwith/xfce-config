use serde::{de, ser};
use std::{collections::BTreeMap, iter::FromIterator};

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
        // TODO: implement this without allocating a Vec
        Vec::<T>::deserialize(deserializer)
            .map(|items| items.into_iter().collect::<Self>())
    }
}
