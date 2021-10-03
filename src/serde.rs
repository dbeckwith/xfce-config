use serde::de;
use std::collections::BTreeMap;

pub trait Id {
    type Id: Clone + Ord;

    fn id(&self) -> &Self::Id;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdMap<T>(pub BTreeMap<T::Id, T>)
where
    T: Id;

impl<'de, T> de::Deserialize<'de> for IdMap<T>
where
    T: de::Deserialize<'de> + Id,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<T>::deserialize(deserializer)
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| (item.id().clone(), item))
                    .collect::<BTreeMap<_, _>>()
            })
            .map(Self)
    }
}
