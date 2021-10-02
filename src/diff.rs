use std::{borrow::Cow, collections::BTreeMap};

pub trait Diff {
    type Patch: Patch;

    /// Computes a one-way diff that when applied to `self` would yield `other`.
    fn diff(&self, other: &Self) -> Self::Patch;
}

pub trait Patch {
    fn is_empty(&self) -> bool;
}

pub trait PrimDiff: PartialEq {}

#[derive(Debug)]
pub struct PrimPatch<T> {
    value: Option<T>,
}

impl<T> Diff for T
where
    T: PrimDiff + Clone,
{
    type Patch = PrimPatch<T>;

    fn diff(&self, other: &Self) -> Self::Patch {
        PrimPatch {
            value: (self != other).then(|| other.clone()),
        }
    }
}

impl<T> Patch for PrimPatch<T> {
    fn is_empty(&self) -> bool {
        self.value.is_none()
    }
}

impl PrimDiff for bool {}
impl PrimDiff for u32 {}
impl PrimDiff for i32 {}
impl PrimDiff for f64 {}
impl PrimDiff for Cow<'_, str> {}

#[derive(Debug)]
pub struct VecPatch<T>
where
    T: Diff,
{
    changed: BTreeMap<usize, T::Patch>,
    added: Vec<T>,
}

impl<T> Diff for Vec<T>
where
    T: Clone + Diff,
{
    type Patch = VecPatch<T>;

    fn diff(&self, other: &Self) -> Self::Patch {
        let mut other_elements = other.iter();
        let changed = self
            .iter()
            .zip(other_elements.by_ref())
            .enumerate()
            .filter_map(|(idx, (self_element, other_element))| {
                let patch = self_element.diff(other_element);
                (!patch.is_empty()).then(|| (idx, patch))
            })
            .collect::<BTreeMap<_, _>>();
        let added = other_elements.cloned().collect::<Vec<_>>();
        VecPatch { changed, added }
    }
}

impl<T> Patch for VecPatch<T>
where
    T: Diff,
{
    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty()
    }
}

#[derive(Debug)]
pub struct BTreeMapPatch<K, V>
where
    V: Diff,
{
    changed: BTreeMap<K, V::Patch>,
    added: BTreeMap<K, V>,
}

impl<K, V> Diff for BTreeMap<K, V>
where
    K: Clone + Eq + Ord,
    V: Clone + Diff,
{
    type Patch = BTreeMapPatch<K, V>;

    fn diff(&self, other: &Self) -> Self::Patch {
        let mut changed = BTreeMap::new();
        let mut added = BTreeMap::new();
        for (key, other_value) in other {
            if let Some(self_value) = self.get(key) {
                let patch = self_value.diff(other_value);
                if !patch.is_empty() {
                    changed.insert(key.clone(), patch);
                }
            } else {
                added.insert(key.clone(), other_value.clone());
            }
        }
        BTreeMapPatch { changed, added }
    }
}

impl<K, V> Patch for BTreeMapPatch<K, V>
where
    V: Diff,
{
    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty()
    }
}
