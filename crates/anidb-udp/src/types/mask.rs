//! The building blocks of the mask tables: [`Kind`], [`Spec`], the
//! [`Field`] trait, [`FieldSet`] and the decoded [`Value`]. The tables of the
//! FILE fields are in [`super::fields`], the one of ANIME in [`super::anime`].
//! The decoding is in [`crate::mask`].

use std::cmp::Reverse;
use std::marker::PhantomData;
use std::ops::Range;

use super::dateflag::DateFlag;
use super::fields::{AnimeField, FileField};

/// The type of a field's value after decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A number. An empty value is `0`.
    Int,
    /// Text, kept as sent.
    Str,
    /// A `1` or `0` flag.
    Bool,
    /// Numbers separated by the given character. Empty items are dropped.
    IntList(char),
    /// Text items separated by the given character. Empty items are dropped.
    StrList(char),
    /// The flags of a date, a bit field: see [`DateFlag`].
    DateFlags,
}

/// One row of a mask table: where a field sits in the mask and how it decodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec<F> {
    /// The field this row describes.
    pub field: F,
    /// The mask byte the bit belongs to, 0 to 8 for the FILE command.
    pub byte: u8,
    /// The bit value within that byte, from `0x80` down to `0x01`.
    pub bit: u8,
    /// The field name.
    pub name: &'static str,
    /// How the value decodes.
    pub kind: Kind,
}

impl<F> Spec<F> {
    /// Builds one table row.
    #[must_use]
    pub const fn new(field: F, byte: u8, bit: u8, name: &'static str, kind: Kind) -> Self {
        Self {
            field,
            byte,
            bit,
            name,
            kind,
        }
    }
}

/// A set of fields of one mask, such as the fmask.
pub trait Field: Copy + Ord + std::fmt::Debug + 'static {
    /// The mask bytes this set covers.
    const BYTES: Range<u8>;
    /// Every field of the set, in table order.
    const TABLE: &'static [Spec<Self>];
}

/// Separator of most lists in the replies.
const APOSTROPHE: char = '\'';
/// Separator of the lists that `AniDB` sends with commas.
const COMMA: char = ',';

pub(crate) const STR_LIST: Kind = Kind::StrList(APOSTROPHE);
pub(crate) const INT_LIST: Kind = Kind::IntList(APOSTROPHE);
pub(crate) const STR_CSV: Kind = Kind::StrList(COMMA);
pub(crate) const INT_CSV: Kind = Kind::IntList(COMMA);

/// The fmask fields a FILE command asks for.
pub type FileFields = FieldSet<FileField>;

/// The amask fields a FILE command asks for.
pub type AnimeFields = FieldSet<AnimeField>;

/// A set of fields of one mask, stored as one bit per table row.
pub struct FieldSet<F> {
    bits: u64,
    marker: PhantomData<F>,
}

// Manual impls: the derives would require `F` to implement the traits as well,
// and the set itself does not need that.
impl<F> Clone for FieldSet<F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for FieldSet<F> {}

impl<F> PartialEq for FieldSet<F> {
    fn eq(&self, other: &Self) -> bool {
        self.bits == other.bits
    }
}

impl<F> Eq for FieldSet<F> {}

impl<F: Field> Default for FieldSet<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Field> std::fmt::Debug for FieldSet<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<F: Field> FieldSet<F> {
    /// The empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bits: 0,
            marker: PhantomData,
        }
    }

    /// Adds a field. Adding a field twice has no effect.
    pub fn insert(&mut self, field: F) {
        self.bits |= 1 << Self::row(field);
    }

    /// Whether the field is in the set.
    #[must_use]
    pub fn contains(&self, field: F) -> bool {
        self.bits & (1 << Self::row(field)) != 0
    }

    /// The number of fields in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bits.count_ones() as usize
    }

    /// Whether the set has no fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    /// The fields in the order the reply holds them.
    pub fn iter(&self) -> impl Iterator<Item = F> + '_ {
        self.specs().into_iter().map(|spec| spec.field)
    }

    /// The mask bytes of this set as lowercase hex, as sent in `fmask=` or
    /// `amask=`. Bytes without a selected field are `00`.
    #[must_use]
    pub fn to_hex(&self) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut hex = String::with_capacity(2 * F::BYTES.len());
        for byte in F::BYTES {
            let value = self
                .selected()
                .filter(|s| s.byte == byte)
                .fold(0, |acc, s| acc | s.bit);
            hex.push(char::from(DIGITS[usize::from(value >> 4)]));
            hex.push(char::from(DIGITS[usize::from(value & 0x0f)]));
        }
        hex
    }

    /// Position of a field in the table, which is its bit.
    fn row(field: F) -> u32 {
        // NOTE: every field of `F` has a row in `F::TABLE`, and a table has fewer than 64 rows.
        let position = F::TABLE
            .iter()
            .position(|spec| spec.field == field)
            .expect("every field has a row in its table");
        u32::try_from(position).expect("a table has fewer than 64 rows")
    }

    /// Rows of the selected fields, in the table order.
    fn selected(self) -> impl Iterator<Item = &'static Spec<F>> {
        let bits = self.bits;
        F::TABLE
            .iter()
            .enumerate()
            .filter(move |(row, _)| bits & (1 << row) != 0)
            .map(|(_, spec)| spec)
    }

    /// Rows of the selected fields in reply order: byte ascending, high bit
    /// first.
    pub(crate) fn specs(self) -> Vec<&'static Spec<F>> {
        let mut specs: Vec<_> = self.selected().collect();
        specs.sort_by_key(|spec| (spec.byte, Reverse(spec.bit)));
        specs
    }
}

impl<F: Field> FromIterator<F> for FieldSet<F> {
    fn from_iter<I: IntoIterator<Item = F>>(iter: I) -> Self {
        let mut set = Self::new();
        for field in iter {
            set.insert(field);
        }
        set
    }
}

/// A decoded field value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A number.
    Int(i64),
    /// Text.
    Str(String),
    /// A list of numbers.
    Ints(Vec<i64>),
    /// A list of text items.
    Strs(Vec<String>),
    /// A flag.
    Bool(bool),
    /// The flags of a date.
    DateFlags(Vec<DateFlag>),
}

impl Value {
    /// The date flags, if this is a [`Value::DateFlags`].
    #[must_use]
    pub fn as_dateflags(&self) -> Option<&[DateFlag]> {
        match self {
            Self::DateFlags(flags) => Some(flags),
            _ => None,
        }
    }

    /// The number, if this is an [`Value::Int`].
    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// The text, if this is a [`Value::Str`].
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(value) => Some(value),
            _ => None,
        }
    }

    /// The numbers, if this is a [`Value::Ints`].
    #[must_use]
    pub fn as_ints(&self) -> Option<&[i64]> {
        match self {
            Self::Ints(items) => Some(items),
            _ => None,
        }
    }

    /// The text items, if this is a [`Value::Strs`].
    #[must_use]
    pub fn as_strs(&self) -> Option<&[String]> {
        match self {
            Self::Strs(items) => Some(items),
            _ => None,
        }
    }

    /// The flag, if this is a [`Value::Bool`].
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }
}
