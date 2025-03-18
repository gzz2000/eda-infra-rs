//! Implementation of a tree-like hierarchical name structure.

use std::fmt;
use std::hash::Hash;
use std::sync::Arc;
use compact_str::CompactString;
use itertools::Itertools;
use dyn_iter::{ DynIter, IntoDynIterator };

/// Hierarchical name representation.
#[derive(PartialEq, Eq, Clone)]
pub struct HierName {
    /// Name of the current layer
    pub cur: CompactString,
    /// Name of the parent layers.
    pub prev: Option<Arc<HierName>>,
}

/// Reverse iterator of a [`HierName`], yielding cell names
/// from the bottom to the top module.
pub struct HierNameRevIter<'i>(Option<&'i HierName>);

impl<'i> Iterator for HierNameRevIter<'i> {
    type Item = &'i CompactString;

    #[inline]
    fn next(&mut self) -> Option<&'i CompactString> {
        let name = self.0?;
        if name.cur.len() == 0 {
            return None
        }
        let ret = &name.cur;
        self.0 = name.prev.as_ref().map(|a| a.as_ref());
        Some(ret)
    }
}

impl<'i> IntoIterator for &'i HierName {
    type Item = &'i CompactString;
    type IntoIter = HierNameRevIter<'i>;

    #[inline]
    fn into_iter(self) -> HierNameRevIter<'i> {
        HierNameRevIter(Some(self))
    }
}

/// Hashing a HierName.
/// 
/// Our guarantee here is that
/// `Hash(HierName[a/b/c]) :== Hash(c, b, a)`.
/// 
/// This is essential for different HierName implementations
/// to agree with each other on hash values.
/// One can find a tricky example in SPEF parser's HierName.
impl Hash for HierName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for s in self.iter() {
            s.hash(state);
        }
    }
}

impl HierName {
    #[inline]
    pub fn single(cur: CompactString) -> Self {
        HierName { cur, prev: None }
    }

    #[inline]
    pub const fn empty() -> Self {
        HierName { cur: CompactString::new_inline(""), prev: None }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.prev.is_none() && self.cur.len() == 0
    }

    #[inline]
    pub fn iter(&self) -> HierNameRevIter {
        (&self).into_iter()
    }

    /// build a hiername using a top-down iterator.
    /// ```
    /// # use netlistdb::HierName;
    /// # use compact_str::CompactString;
    /// assert_eq!(format!("{:?}", HierName::from_topdown_hier_iter(
    ///     ["abc", "def"]
    /// )), "HierName(abc/def)");
    /// ```
    #[inline]
    pub fn from_topdown_hier_iter<I: Into<CompactString>>(
        iter: impl IntoIterator<Item = I>
    ) -> HierName {
        let mut ret = HierName::empty();
        for ident in iter {
            if ret.is_empty() {
                ret = HierName { cur: ident.into(), prev: None };
            }
            else {
                ret = HierName { cur: ident.into(),
                                 prev: Some(Arc::from(ret)) };
            }
        }
        ret
    }
}

impl fmt::Display for HierName {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(v) = &self.prev {
            write!(f, "{}/", v)?;
        }
        write!(f, "{}", self.cur)
    }
}

impl fmt::Debug for HierName {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HierName({})", self)
    }
}

#[test]
fn test_hier_name() {
    let h1 = Arc::from(HierName{
        cur: CompactString::new_inline("top"),
        prev: None
    });
    let h2 = Arc::from(HierName{
        cur: CompactString::new_inline("mod1"),
        prev: Some(h1.clone())
    });
    let h3 = Arc::from(HierName{
        cur: CompactString::new_inline("leaf1"),
        prev: Some(h2.clone())
    });
    let h3_ = Arc::from(HierName{
        cur: CompactString::new_inline("leaf2"),
        prev: Some(h2.clone())
    });
    assert_eq!(format!("{}", h3), "top/mod1/leaf1");
    assert_eq!(format!("{:?}", h3_), "HierName(top/mod1/leaf2)");
    assert_eq!(h3.iter().map(|a| a.as_ref()).collect::<Vec<&str>>(),
               vec!["leaf1", "mod1", "top"]);
}

/// We use this to unify netlistdb::HierName and other
/// implementations, such as
/// spefparse::HierName, sdfparse::SDFPath, etc.
/// 
/// See a great post on this:
/// <https://stackoverflow.com/questions/45786717/how-to-implement-hashmap-with-two-keys>
pub trait GeneralHierName {
    fn ident_iter(&self) -> DynIter<&str>;

    /// Format any hier name to `a/b/c`-like `String`, for debugging.
    fn dbg_fmt_hier(&self) -> String {
        let mut v: Vec<_> = self.ident_iter().collect();
        v.reverse();
        format!("{}", v.iter().format("/"))
    }
}

impl<T, S> GeneralHierName for T
where for<'i> &'i T: IntoIterator<Item = &'i S>,
      T: Hash,
      S: AsRef<str>
{
    #[inline]
    fn ident_iter(&self) -> DynIter<&str> {
        self.into_iter().map(|a| a.as_ref()).into_dyn_iter()
    }
}

impl Hash for dyn GeneralHierName + '_ {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for s in self.ident_iter() {
            s.hash(state);
        }
    }
}

// Below won't work because it trips over the orphan rule.
// See this answer: https://stackoverflow.com/a/63131661/11815215
// 
// impl<T: GeneralHierName> std::borrow::Borrow<dyn GeneralHierName> for T {
//     #[inline]
//     fn borrow(&self) -> &dyn GeneralHierName {
//         self
//     }
// }

impl<'i> std::borrow::Borrow<dyn GeneralHierName + 'i> for HierName {
    #[inline]
    fn borrow(&self) -> &(dyn GeneralHierName + 'i) {
        self
    }
}

impl<'i> std::borrow::Borrow<dyn GeneralHierName + 'i> for &'i HierName {
    #[inline]
    fn borrow(&self) -> &(dyn GeneralHierName + 'i) {
        *self
    }
}

impl PartialEq for dyn GeneralHierName + '_ {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.ident_iter().eq(other.ident_iter())
    }
}

impl Eq for dyn GeneralHierName + '_ {}

/// We use this to unify the hash query of pin names.
/// A pin name consists of (cell hierarchy, pin type, bus id)
pub trait GeneralPinName {
    fn hierarchy(&self) -> &dyn GeneralHierName;
    fn pin_type(&self) -> &str;
    fn bus_id(&self) -> Option<isize>;

    /// Format any pin name to `a/b/c:d[0]`-like `String`, for debugging.
    fn dbg_fmt_pin(&self) -> String {
        let hier = self.hierarchy().dbg_fmt_hier();
        format!("{}{}{}",
                match hier.is_empty() {
                    true => "".to_string(),
                    false => format!("{}:", hier)
                },
                self.pin_type(),
                match self.bus_id() {
                    None => "".to_string(),
                    Some(id) => format!("[{}]", id)
                })
    }
}

impl<C, S> GeneralPinName for (C, S, Option<isize>)
where C: GeneralHierName, S: AsRef<str>
{
    #[inline]
    fn hierarchy(&self) -> &dyn GeneralHierName {
        &self.0
    }

    #[inline]
    fn pin_type(&self) -> &str {
        self.1.as_ref()
    }

    #[inline]
    fn bus_id(&self) -> Option<isize> {
        self.2
    }
}

impl Hash for dyn GeneralPinName + '_ {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // warning: this relies on the tuple hash implementation (https://doc.rust-lang.org/src/core/hash/mod.rs.html) to hash every elements one by one.
        for s in self.hierarchy().ident_iter() {
            s.hash(state);
        }
        self.pin_type().hash(state);
        self.bus_id().hash(state);
    }
}

impl<'i> std::borrow::Borrow<dyn GeneralPinName + 'i>
    for (HierName, CompactString, Option<isize>)
{
    #[inline]
    fn borrow(&self) -> &(dyn GeneralPinName + 'i) {
        self
    }
}

impl<'i, 'j> std::borrow::Borrow<dyn GeneralPinName + 'i>
    for &'j (HierName, CompactString, Option<isize>)
{
    #[inline]
    fn borrow(&self) -> &(dyn GeneralPinName + 'i) {
        *self
    }
}

impl PartialEq for dyn GeneralPinName + '_ {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.bus_id() == other.bus_id() &&
            self.pin_type() == other.pin_type() &&
            self.hierarchy() == other.hierarchy()
    }
}

impl Eq for dyn GeneralPinName + '_ {}

/// this struct is used to zero-copy refer to a general
/// pin tuple, for map lookup.
pub struct RefPinName<'a, 'b, T: GeneralHierName>(
    pub &'a T, pub &'b str, pub Option<isize>
);

impl<'a, 'b, T: GeneralHierName + Hash> Hash for RefPinName<'a, 'b, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
        self.2.hash(state);
    }
}

impl<'a, 'b, T: GeneralHierName> GeneralPinName for RefPinName<'a, 'b, T> {
    #[inline]
    fn hierarchy(&self) -> &dyn GeneralHierName {
        &*self.0
    }


    #[inline]
    fn pin_type(&self) -> &str {
        self.1.as_ref()
    }

    #[inline]
    fn bus_id(&self) -> Option<isize> {
        self.2
    }
}

/// We use this to unify the hash query of macro pin names
/// A macro pin name consists of (pin type, bus id)
pub trait GeneralMacroPinName {
    fn pin_type(&self) -> &str;
    fn bus_id(&self) -> Option<isize>;

    /// Format any pin name to `a/b/c:d[0]`-like `String`, for debugging.
    fn dbg_fmt_macro_pin(&self) -> String {
        format!("{}{}",
                self.pin_type(),
                match self.bus_id() {
                    None => "".to_string(),
                    Some(id) => format!("[{}]", id)
                })
    }
}

impl<S: AsRef<str>> GeneralMacroPinName for (S, Option<isize>) {
    #[inline]
    fn pin_type(&self) -> &str {
        self.0.as_ref()
    }

    #[inline]
    fn bus_id(&self) -> Option<isize> {
        self.1
    }
}

impl Hash for dyn GeneralMacroPinName + '_ {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // warning: this relies on the tuple hash implementation (https://doc.rust-lang.org/src/core/hash/mod.rs.html) to hash every elements one by one.
        self.pin_type().hash(state);
        self.bus_id().hash(state);
    }
}

impl<'i> std::borrow::Borrow<dyn GeneralMacroPinName + 'i>
    for (CompactString, Option<isize>)
{
    #[inline]
    fn borrow(&self) -> &(dyn GeneralMacroPinName + 'i) {
        self
    }
}

impl<'i, 'j> std::borrow::Borrow<dyn GeneralMacroPinName + 'i>
    for &'j (CompactString, Option<isize>)
{
    #[inline]
    fn borrow(&self) -> &(dyn GeneralMacroPinName + 'i) {
        *self
    }
}

impl PartialEq for dyn GeneralMacroPinName + '_ {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.bus_id() == other.bus_id() &&
            self.pin_type() == other.pin_type()
    }
}

impl Eq for dyn GeneralMacroPinName + '_ {}

/// this struct is used to zero-copy refer to a general
/// macro pin name tuple, for map lookup.
pub struct RefMacroPinName<'a>(
    pub &'a str, pub Option<isize>
);

impl<'a> Hash for RefMacroPinName<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
    }
}

impl<'a> GeneralMacroPinName for RefMacroPinName<'a> {
    #[inline]
    fn pin_type(&self) -> &str {
        self.0
    }

    #[inline]
    fn bus_id(&self) -> Option<isize> {
        self.1
    }
}

#[test]
fn test_general_hier_hash() {
    let h1 = Arc::from(HierName{
        cur: CompactString::new_inline("top"),
        prev: None
    });
    let h2 = Arc::from(HierName{
        cur: CompactString::new_inline("mod1"),
        prev: Some(h1.clone())
    });
    let h3 = Arc::from(HierName{
        cur: CompactString::new_inline("leaf1"),
        prev: Some(h2.clone())
    });
    fn hash(x: impl Hash) -> u64 {
        use std::hash::Hasher;
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        x.hash(&mut hasher);
        hasher.finish()
    }
    assert_eq!(hash(h3.clone()), hash(("leaf1", "mod1", "top")));
    assert_eq!(hash(h3.clone()), hash(&["leaf1", "mod1", "top"] as &dyn GeneralHierName));
}
