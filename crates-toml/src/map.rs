use std::collections::hash_map::Entry::*;
use std::collections::HashMap;

use crate::onevec::OneVec;
use crate::parse::{
    ArrayEntry, Assignment, Ast, BoolVal, DateTimeVal, DottedIdent, FloatVal, Ident, InlineArray,
    InlineArrayValue, InlineTableAssignment, IntVal, Key, StringVal, Table, Value,
};
use crate::{Ctx, Error, Span};

pub mod simple;
#[cfg(test)]
mod test;

#[derive(Debug, PartialEq)]
pub struct MapTable<'a> {
    inner: HashMap<&'a str, MapTableEntry<'a>>,
}

impl<'a> MapTable<'a> {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn get(&self, key: &'a str) -> Option<&MapTableEntry<'a>> {
        self.inner.get(key)
    }

    pub fn from_pairs(pairs: impl IntoIterator<Item = (&'a str, MapTableEntry<'a>)>) -> Self {
        Self {
            inner: HashMap::from_iter(pairs),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&&str, &MapTableEntry<'a>)> {
        self.inner.iter()
    }
}

impl<'a> IntoIterator for MapTable<'a> {
    type Item = (&'a str, MapTableEntry<'a>);
    type IntoIter = std::collections::hash_map::IntoIter<&'a str, MapTableEntry<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

#[derive(Debug, PartialEq)]
pub struct MapTableEntry<'a> {
    pub node: MapNode<'a>,
    /// References to the actual representations inside the toml file.
    pub reprs: OneVec<MapTableEntryRepr<'a>>,
}

impl<'a> MapTableEntry<'a> {
    fn from_one(node: MapNode<'a>, repr: MapTableEntryRepr<'a>) -> Self {
        Self {
            node,
            reprs: OneVec::new(repr),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapTableEntryRepr<'a> {
    pub key: MapTableKeyRepr<'a>,
    pub kind: MapTableEntryReprKind<'a>,
}

impl<'a> MapTableEntryRepr<'a> {
    fn new(key: MapTableKeyRepr<'a>, kind: MapTableEntryReprKind<'a>) -> Self {
        Self { key, kind }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MapTableEntryReprKind<'a> {
    Table(&'a Table<'a>),
    ArrayEntry(&'a ArrayEntry<'a>),
    ToplevelAssignment(&'a Assignment<'a>),
    InlineTableAssignment(&'a InlineTableAssignment<'a>),
}

impl MapTableEntryReprKind<'_> {
    #[inline]
    pub fn span(&self) -> Span {
        match self {
            MapTableEntryReprKind::Table(t) => t.span(),
            MapTableEntryReprKind::ArrayEntry(a) => a.span(),
            MapTableEntryReprKind::ToplevelAssignment(a) => a.span(),
            MapTableEntryReprKind::InlineTableAssignment(a) => a.span(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapTableKeyRepr<'a> {
    One(&'a Ident<'a>),
    Dotted(u32, &'a [DottedIdent<'a>]),
}

impl<'a> MapTableKeyRepr<'a> {
    /// The [`Ident`] responsible for declaring a table, might be an identifier in a dotted key.
    pub fn repr_ident(&self) -> &'a Ident<'a> {
        match self {
            MapTableKeyRepr::One(i) => i,
            MapTableKeyRepr::Dotted(idx, idents) => &idents[*idx as usize].ident,
        }
    }

    pub fn is_last_ident(&self) -> bool {
        match self {
            MapTableKeyRepr::One(_) => true,
            MapTableKeyRepr::Dotted(idx, idents) => *idx as usize == idents.len() - 1,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MapArray<'a> {
    Toplevel(MapArrayToplevel<'a>),
    Inline(MapArrayInline<'a>),
}

#[derive(Debug, PartialEq)]
pub struct MapArrayToplevel<'a> {
    inner: OneVec<MapArrayToplevelEntry<'a>>,
}

impl<'a> MapArrayToplevel<'a> {
    pub fn new(node: MapTable<'a>, repr: &'a ArrayEntry<'a>) -> Self {
        Self {
            inner: OneVec::new(MapArrayToplevelEntry::new(node, repr)),
        }
    }

    fn push(&mut self, node: MapTable<'a>, repr: &'a ArrayEntry<'a>) {
        self.inner.push(MapArrayToplevelEntry::new(node, repr));
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

#[derive(Debug, PartialEq)]
pub struct MapArrayToplevelEntry<'a> {
    pub node: MapTable<'a>,
    pub repr: &'a ArrayEntry<'a>,
}

impl<'a> MapArrayToplevelEntry<'a> {
    pub fn new(node: MapTable<'a>, repr: &'a ArrayEntry<'a>) -> Self {
        Self { node, repr }
    }
}

#[derive(Debug, PartialEq)]
pub struct MapArrayInline<'a> {
    pub repr: &'a InlineArray<'a>,
    inner: Vec<MapArrayInlineEntry<'a>>,
}

impl<'a> MapArrayInline<'a> {
    pub fn new(repr: &'a InlineArray<'a>) -> Self {
        Self {
            repr,
            inner: Vec::new(),
        }
    }

    pub fn from_iter(
        repr: &'a InlineArray<'a>,
        iter: impl IntoIterator<Item = MapArrayInlineEntry<'a>>,
    ) -> Self {
        Self {
            repr,
            inner: Vec::from_iter(iter),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn get(&'a self, idx: usize) -> Option<&'a MapArrayInlineEntry<'a>> {
        self.inner.get(idx)
    }

    pub fn iter(&'a self) -> impl Iterator<Item = &'a MapArrayInlineEntry<'a>> {
        self.inner.iter()
    }
}

impl<'a, I: std::slice::SliceIndex<[MapArrayInlineEntry<'a>]>> std::ops::Index<I>
    for MapArrayInline<'a>
{
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        std::ops::Index::index(&*self.inner, index)
    }
}

impl<'a, I: std::slice::SliceIndex<[MapArrayInlineEntry<'a>]>> std::ops::IndexMut<I>
    for MapArrayInline<'a>
{
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        std::ops::IndexMut::index_mut(&mut *self.inner, index)
    }
}

#[derive(Debug, PartialEq)]
pub struct MapArrayInlineEntry<'a> {
    pub node: MapNode<'a>,
    pub repr: &'a InlineArrayValue<'a>,
}

impl<'a> MapArrayInlineEntry<'a> {
    pub fn new(node: MapNode<'a>, repr: &'a InlineArrayValue<'a>) -> Self {
        Self { node, repr }
    }
}

#[derive(Debug, PartialEq)]
pub enum MapNode<'a> {
    Table(MapTable<'a>),
    Array(MapArray<'a>),
    Scalar(Scalar<'a>),
}

#[derive(Debug, PartialEq)]
pub enum Scalar<'a> {
    String(&'a StringVal<'a>),
    Int(&'a IntVal<'a>),
    Float(&'a FloatVal<'a>),
    Bool(&'a BoolVal),
    DateTime(&'a DateTimeVal<'a>),
    Invalid(&'a str, Span),
}

impl Scalar<'_> {
    #[inline]
    pub fn span(&self) -> Span {
        match self {
            Scalar::String(s) => s.lit_span,
            Scalar::Int(i) => i.lit_span,
            Scalar::Float(f) => f.lit_span,
            Scalar::Bool(b) => b.lit_span,
            Scalar::DateTime(d) => d.lit_span,
            Scalar::Invalid(_, span) => *span,
        }
    }
}

struct Mapper {
    current_path: String,
}

impl Mapper {
    fn new() -> Self {
        Self {
            current_path: String::new(),
        }
    }

    #[inline]
    fn with_path<T>(&mut self, f: impl FnOnce(&mut Mapper) -> T) -> T {
        let len = self.current_path.len();
        let val = f(self);
        self.current_path.truncate(len);
        val
    }

    #[inline]
    fn with_key<T>(&mut self, key: &str, f: impl FnOnce(&mut Mapper) -> T) -> T {
        let len = self.current_path.len();
        self.push_key(key);
        let val = f(self);
        self.current_path.truncate(len);
        val
    }

    #[inline]
    fn with_index<T>(&mut self, index: usize, f: impl FnOnce(&mut Mapper) -> T) -> T {
        let len = self.current_path.len();
        self.push_index(index);
        let val = f(self);
        self.current_path.truncate(len);
        val
    }

    fn push_key(&mut self, key: &str) {
        if !self.current_path.is_empty() {
            self.current_path.push('.');
        }
        self.current_path.push_str(key);
    }

    fn push_index(&mut self, index: usize) {
        use std::fmt::Write;
        _ = write!(&mut self.current_path, "[{index}]");
    }

    fn path(&self) -> Option<Box<str>> {
        if self.current_path.is_empty() {
            None
        } else {
            Some(self.current_path.clone().into_boxed_str())
        }
    }

    fn joined_path(&self, key: &str) -> Box<str> {
        if self.current_path.is_empty() {
            key.into()
        } else {
            format!("{}.{}", self.current_path, key).into_boxed_str()
        }
    }
}

/// Value to be lazily mapped and inserted
enum InsertValue<'a> {
    Value(&'a Value<'a>),
    TableAssignments(&'a [Assignment<'a>]),
}

impl Ctx {
    pub fn map<'a>(&mut self, asts: &'a [Ast<'a>]) -> MapTable<'a> {
        let mapper = &mut Mapper::new();
        let mut root = MapTable::new();
        for a in asts {
            match a {
                Ast::Assignment(assignment) => {
                    let repr_kind = MapTableEntryReprKind::ToplevelAssignment(assignment);
                    self.insert_node_at_path(
                        mapper,
                        &mut root,
                        &assignment.key,
                        InsertValue::Value(&assignment.val),
                        repr_kind,
                    );
                }
                Ast::Table(table) => {
                    let Some(key) = &table.header.key else {
                        continue;
                    };

                    let repr_kind = MapTableEntryReprKind::Table(table);
                    self.insert_node_at_path(
                        mapper,
                        &mut root,
                        key,
                        InsertValue::TableAssignments(&table.assignments),
                        repr_kind,
                    );
                }
                Ast::Array(array_entry) => {
                    let Some(key) = &array_entry.header.key else {
                        continue;
                    };
                    self.insert_array_entry_at_path(mapper, &mut root, key, array_entry);
                }
                Ast::Comment(_) => (),
            }
        }
        root
    }

    fn map_insert_value<'a>(&mut self, mapper: &mut Mapper, value: InsertValue<'a>) -> MapNode<'a> {
        match value {
            InsertValue::Value(value) => self.map_value(mapper, value),
            InsertValue::TableAssignments(assignments) => {
                let mut map = MapTable::new();
                self.insert_top_level_assignments(mapper, &mut map, assignments);
                MapNode::Table(map)
            }
        }
    }

    fn map_value<'a>(&mut self, mapper: &mut Mapper, value: &'a Value<'a>) -> MapNode<'a> {
        match value {
            Value::String(s) => MapNode::Scalar(Scalar::String(s)),
            Value::Int(i) => MapNode::Scalar(Scalar::Int(i)),
            Value::Float(f) => MapNode::Scalar(Scalar::Float(f)),
            Value::Bool(b) => MapNode::Scalar(Scalar::Bool(b)),
            Value::DateTime(d) => MapNode::Scalar(Scalar::DateTime(d)),
            Value::InlineTable(table) => {
                let mut map = MapTable::new();
                for assignment in table.assignments.iter() {
                    let repr_kind = MapTableEntryReprKind::InlineTableAssignment(assignment);
                    self.insert_node_at_path(
                        mapper,
                        &mut map,
                        &assignment.assignment.key,
                        InsertValue::Value(&assignment.assignment.val),
                        repr_kind,
                    );
                }
                MapNode::Table(map)
            }
            Value::InlineArray(inline_array) => {
                let entries = (inline_array.values.iter().enumerate()).map(|(index, value)| {
                    let node = mapper.with_index(index, |mapper| {
                        self.map_value(mapper, &value.val) //
                    });
                    MapArrayInlineEntry::new(node, value)
                });
                let array = MapArrayInline::from_iter(inline_array, entries);
                MapNode::Array(MapArray::Inline(array))
            }
            Value::Invalid(s, r) => MapNode::Scalar(Scalar::Invalid(s, *r)),
        }
    }

    fn insert_node_at_path<'a>(
        &mut self,
        mapper: &mut Mapper,
        map: &mut MapTable<'a>,
        key: &'a Key<'a>,
        value: InsertValue<'a>,
        repr_kind: MapTableEntryReprKind<'a>,
    ) {
        mapper.with_path(|mapper| {
            let idents = match &key {
                Key::One(i) => {
                    let key_repr = MapTableKeyRepr::One(i);
                    let repr = MapTableEntryRepr::new(key_repr, repr_kind);
                    let res = self.insert_node(mapper, map, i.text(), value, repr);
                    if let Err(e) = res {
                        self.error(e);
                    }
                    return;
                }
                Key::Dotted(idents) => idents,
            };

            let [other @ .., last] = idents.as_slice() else {
                unreachable!()
            };
            let mut current = map;
            for (i, o) in other.iter().enumerate() {
                let entry = match current.inner.entry(o.ident.text()) {
                    Occupied(occupied) => occupied.into_mut(),
                    Vacant(vacant) => {
                        let offset = i + 1;
                        for i in idents[offset..].iter() {
                            mapper.push_key(i.ident.lit);
                        }
                        let mut node = self.map_insert_value(mapper, value);

                        for (j, o) in idents[offset..].iter().enumerate().rev() {
                            let key_idx = (offset + j) as u32;
                            let key_repr = MapTableKeyRepr::Dotted(key_idx, idents);
                            let repr = MapTableEntryRepr::new(key_repr, repr_kind);
                            node = MapNode::Table(MapTable::from_pairs([(
                                o.ident.text(),
                                MapTableEntry::from_one(node, repr),
                            )]));
                        }

                        let key_repr = MapTableKeyRepr::Dotted(i as u32, idents);
                        let repr = MapTableEntryRepr::new(key_repr, repr_kind);
                        vacant.insert(MapTableEntry::from_one(node, repr));
                        return;
                    }
                };

                let next =
                    match get_table_to_extend(mapper, &mut entry.node, &entry.reprs, &o.ident) {
                        Ok(t) => t,
                        Err(e) => return self.error(e),
                    };

                let key_repr = MapTableKeyRepr::Dotted(i as u32, idents);
                let repr = MapTableEntryRepr::new(key_repr, repr_kind);
                entry.reprs.push(repr);
                current = next;
                mapper.push_key(o.ident.lit);
            }

            let key_repr = MapTableKeyRepr::Dotted((idents.len() - 1) as u32, idents);
            let repr = MapTableEntryRepr::new(key_repr, repr_kind);

            let res = self.insert_node(mapper, current, last.ident.text(), value, repr);
            if let Err(e) = res {
                self.error(e);
            }
        });
    }

    fn insert_node<'a>(
        &mut self,
        mapper: &mut Mapper,
        map: &mut MapTable<'a>,
        key: &'a str,
        value: InsertValue<'a>,
        repr: MapTableEntryRepr<'a>,
    ) -> Result<(), Error> {
        use std::collections::hash_map::Entry::*;

        let existing_entry = match map.inner.entry(key) {
            Occupied(occupied) => occupied.into_mut(),
            Vacant(vacant) => {
                let node = mapper.with_key(key, |mapper| self.map_insert_value(mapper, value));
                vacant.insert(MapTableEntry::from_one(node, repr));
                return Ok(());
            }
        };

        let InsertValue::TableAssignments(assignments) = value else {
            return Err(duplicate_key_error(
                mapper,
                repr.key.repr_ident(),
                existing_entry,
                &repr.key,
            ));
        };
        let existing_table = match &mut existing_entry.node {
            MapNode::Table(t) => t,
            MapNode::Array(_) | MapNode::Scalar(_) => {
                return Err(duplicate_key_error(
                    mapper,
                    repr.key.repr_ident(),
                    existing_entry,
                    &repr.key,
                ));
            }
        };
        for existing_repr in existing_entry.reprs.iter() {
            match existing_repr.kind {
                MapTableEntryReprKind::Table(_) if !existing_repr.key.is_last_ident() => {
                    // allow super tables, that are declared out of order
                }
                MapTableEntryReprKind::Table(_)
                | MapTableEntryReprKind::ArrayEntry(_)
                | MapTableEntryReprKind::ToplevelAssignment(_)
                | MapTableEntryReprKind::InlineTableAssignment(_) => {
                    return Err(duplicate_key_error(
                        mapper,
                        repr.key.repr_ident(),
                        existing_entry,
                        &repr.key,
                    ))
                }
            }
        }

        // extend existing table with items from super table
        mapper.with_key(key, |mapper| {
            self.insert_top_level_assignments(mapper, existing_table, assignments);
            existing_entry.reprs.push(repr);
        });

        Ok(())
    }

    fn insert_array_entry_at_path<'a>(
        &mut self,
        mapper: &mut Mapper,
        map: &mut MapTable<'a>,
        key: &'a Key<'a>,
        array_entry: &'a ArrayEntry<'a>,
    ) {
        mapper.with_path(|mapper| {
            let idents = match key {
                Key::One(i) => {
                    let key_repr = MapTableKeyRepr::One(i);
                    let res = self.insert_array_entry(mapper, map, i.text(), key_repr, array_entry);
                    if let Err(e) = res {
                        self.error(e);
                    }
                    return;
                }
                Key::Dotted(idents) => idents,
            };

            let [other @ .., last] = idents.as_slice() else {
                unreachable!()
            };
            let mut current = map;
            for (i, o) in other.iter().enumerate() {
                let entry = match current.inner.entry(o.ident.text()) {
                    Occupied(occupied) => occupied.into_mut(),
                    Vacant(vacant) => {
                        let offset = i + 1;
                        for i in idents[offset..].iter() {
                            mapper.push_key(i.ident.lit);
                        }
                        mapper.push_index(0);
                        let mut node = MapTable::new();
                        self.insert_top_level_assignments(
                            mapper,
                            &mut node,
                            &array_entry.assignments,
                        );
                        let toplevel_array = MapArrayToplevel::new(node, array_entry);
                        let mut node = MapNode::Array(MapArray::Toplevel(toplevel_array));

                        for (j, o) in idents[offset..].iter().enumerate().rev() {
                            let key_idx = (offset + j) as u32;
                            let key_repr = MapTableKeyRepr::Dotted(key_idx, idents);
                            let repr_kind = MapTableEntryReprKind::ArrayEntry(array_entry);
                            let repr = MapTableEntryRepr::new(key_repr, repr_kind);
                            node = MapNode::Table(MapTable::from_pairs([(
                                o.ident.text(),
                                MapTableEntry::from_one(node, repr),
                            )]));
                        }

                        let key_repr = MapTableKeyRepr::Dotted(i as u32, idents);
                        let repr_kind = MapTableEntryReprKind::ArrayEntry(array_entry);
                        let repr = MapTableEntryRepr::new(key_repr, repr_kind);
                        vacant.insert(MapTableEntry::from_one(node, repr));
                        return;
                    }
                };

                let next =
                    match get_table_to_extend(mapper, &mut entry.node, &entry.reprs, &o.ident) {
                        Ok(t) => t,
                        Err(e) => return self.error(e),
                    };

                let key_repr = MapTableKeyRepr::Dotted(i as u32, idents);
                let repr_kind = MapTableEntryReprKind::ArrayEntry(array_entry);
                let repr = MapTableEntryRepr::new(key_repr, repr_kind);
                entry.reprs.push(repr);
                current = next;
                mapper.push_key(o.ident.lit);
            }

            let key_repr = MapTableKeyRepr::Dotted((idents.len() - 1) as u32, idents);
            let res =
                self.insert_array_entry(mapper, current, last.ident.text(), key_repr, array_entry);
            if let Err(e) = res {
                self.error(e);
            }
        });
    }

    fn insert_array_entry<'a>(
        &mut self,
        mapper: &mut Mapper,
        map: &mut MapTable<'a>,
        key: &'a str,
        key_repr: MapTableKeyRepr<'a>,
        array_entry: &'a ArrayEntry<'a>,
    ) -> Result<(), Error> {
        let repr_kind = MapTableEntryReprKind::ArrayEntry(array_entry);
        let repr = MapTableEntryRepr::new(key_repr, repr_kind);

        match map.inner.entry(key) {
            Occupied(occupied) => {
                let entry = occupied.into_mut();
                let array = match &mut entry.node {
                    MapNode::Array(MapArray::Toplevel(a)) => a,
                    MapNode::Array(MapArray::Inline(_)) => {
                        let path = mapper.joined_path(key);
                        let orig = entry.reprs.first().kind.span();
                        let new = repr.key.repr_ident().lit_span;
                        return Err(Error::CannotExtendInlineArray { path, orig, new });
                    }
                    MapNode::Table(_) | MapNode::Scalar(_) => {
                        return Err(duplicate_key_error(
                            mapper,
                            repr.key.repr_ident(),
                            entry,
                            &repr.key,
                        ));
                    }
                };

                let idx = array.len();
                let mut node = MapTable::new();
                mapper.with_key(key, |mapper| {
                    mapper.with_index(idx, |mapper| {
                        self.insert_top_level_assignments(
                            mapper,
                            &mut node,
                            &array_entry.assignments,
                        );
                    })
                });

                entry.reprs.push(repr);
                array.push(node, array_entry);
            }
            Vacant(vacant) => {
                let mut node = MapTable::new();
                mapper.with_key(key, |mapper| {
                    mapper.with_index(0, |mapper| {
                        self.insert_top_level_assignments(
                            mapper,
                            &mut node,
                            &array_entry.assignments,
                        );
                    })
                });

                let toplevel_array = MapArrayToplevel::new(node, array_entry);
                let node = MapNode::Array(MapArray::Toplevel(toplevel_array));
                vacant.insert(MapTableEntry::from_one(node, repr));
            }
        }

        Ok(())
    }

    fn insert_top_level_assignments<'a>(
        &mut self,
        mapper: &mut Mapper,
        map: &mut MapTable<'a>,
        assignments: &'a [Assignment<'a>],
    ) {
        for assignment in assignments.iter() {
            let repr_kind = MapTableEntryReprKind::ToplevelAssignment(assignment);
            self.insert_node_at_path(
                mapper,
                map,
                &assignment.key,
                InsertValue::Value(&assignment.val),
                repr_kind,
            );
        }
    }
}

fn get_table_to_extend<'a, 'b>(
    mapper: &mut Mapper,
    node: &'b mut MapNode<'a>,
    reprs: &OneVec<MapTableEntryRepr<'a>>,
    ident: &'b Ident<'a>,
) -> Result<&'b mut MapTable<'a>, Error>
where
    'a: 'b,
{
    let next = match node {
        MapNode::Table(t) => t,
        MapNode::Array(MapArray::Toplevel(t)) => {
            // From the toml spec (https://toml.io/en/v1.0.0#array-of-tables):
            // Any reference to an array of tables points to the most recently
            // defined table element of the array. This allows you to define
            // sub-tables, and even sub-arrays of tables, inside the most recent
            // table.
            &mut t.inner.last_mut().node
        }
        MapNode::Array(MapArray::Inline(_)) => {
            let path = mapper.joined_path(ident.lit);
            let orig = reprs.first().kind.span();
            let new = ident.lit_span;
            return Err(Error::CannotExtendInlineArrayAsTable { path, orig, new });
        }
        MapNode::Scalar(_) => {
            return Err(Error::DuplicateKey {
                path: mapper.path(),
                key: ident.lit.to_string().into_boxed_str(),
                orig: reprs.first().key.repr_ident().lit_span,
                duplicate: ident.lit_span,
            });
        }
    };

    for existing in reprs.iter() {
        match &existing.kind {
            MapTableEntryReprKind::Table(_) => (),
            MapTableEntryReprKind::ArrayEntry(_) => (),
            MapTableEntryReprKind::ToplevelAssignment(_) => {
                if existing.key.is_last_ident() {
                    // `next` is an inline table
                    let path = mapper.joined_path(ident.lit);
                    let orig = existing.kind.span();
                    let new = ident.lit_span;
                    return Err(Error::CannotExtendInlineTable { path, orig, new });
                }
            }
            MapTableEntryReprKind::InlineTableAssignment(_) => {
                // we're inside an inline table, which can't be extended
                let path = mapper.joined_path(ident.lit);
                let orig = existing.kind.span();
                let new = ident.lit_span;
                return Err(Error::CannotExtendInlineTable { path, orig, new });
            }
        }
    }

    Ok(next)
}

fn duplicate_key_error(
    mapper: &Mapper,
    ident: &Ident<'_>,
    entry: &MapTableEntry<'_>,
    duplicate: &MapTableKeyRepr<'_>,
) -> Error {
    Error::DuplicateKey {
        path: mapper.path(),
        key: ident.lit.to_string().into_boxed_str(),
        orig: entry.reprs.first().key.repr_ident().lit_span,
        duplicate: duplicate.repr_ident().lit_span,
    }
}