use std::{borrow::Cow, collections::BTreeMap};

pub trait Diff {
    type Patch: Patch;

    /// Computes a one-way diff that when applied to `self` would yield `other`.
    fn diff(&self, other: &Self) -> Self::Patch;
}

pub trait Patch {
    fn is_empty(&self) -> bool;
}

// FIXME: don't store delta for primitives, need complete value for apply

#[derive(Debug)]
pub struct BoolPatch {
    flip: bool,
}

impl Diff for bool {
    type Patch = BoolPatch;

    fn diff(&self, other: &Self) -> Self::Patch {
        BoolPatch {
            flip: self != other,
        }
    }
}

impl Patch for BoolPatch {
    fn is_empty(&self) -> bool {
        !self.flip
    }
}

#[derive(Debug)]
pub struct U32Patch {
    diff: i32,
}

impl Diff for u32 {
    type Patch = U32Patch;

    fn diff(&self, other: &Self) -> Self::Patch {
        U32Patch {
            diff: (*other as i32) - (*self as i32),
        }
    }
}

impl Patch for U32Patch {
    fn is_empty(&self) -> bool {
        self.diff == 0
    }
}

#[derive(Debug)]
pub struct I32Patch {
    diff: i32,
}

impl Diff for i32 {
    type Patch = I32Patch;

    fn diff(&self, other: &Self) -> Self::Patch {
        I32Patch {
            diff: (*other as i32) - (*self as i32),
        }
    }
}

impl Patch for I32Patch {
    fn is_empty(&self) -> bool {
        self.diff == 0
    }
}

#[derive(Debug)]
pub struct F64Patch {
    diff: f64,
}

impl Diff for f64 {
    type Patch = F64Patch;

    fn diff(&self, other: &Self) -> Self::Patch {
        F64Patch { diff: other - self }
    }
}

impl Patch for F64Patch {
    fn is_empty(&self) -> bool {
        self.diff == 0.0
    }
}

#[derive(Debug)]
pub struct CowStrPatch<'a> {
    new: Option<Cow<'a, str>>,
}

impl<'a> Diff for Cow<'a, str> {
    type Patch = CowStrPatch<'a>;

    fn diff(&self, other: &Self) -> Self::Patch {
        CowStrPatch {
            new: (self != other).then(|| other.clone()),
        }
    }
}

impl Patch for CowStrPatch<'_> {
    fn is_empty(&self) -> bool {
        self.new.is_none()
    }
}

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
