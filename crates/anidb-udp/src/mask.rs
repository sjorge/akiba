//! Bit mask tables of the FILE command, and the typed field sets built on them.
//! The tables follow the FILE command of the specification, see the crate root
//! for the link.
//!
//! The FILE command selects its fields with a nine byte mask. Bytes 0 to 4 are
//! the fmask, whose fields describe the file ([`FileField`]). Bytes 5 to 8 are
//! the amask, whose fields describe the anime, episode and group
//! ([`AnimeField`]). Each bit selects one field.
//!
//! The reply holds the selected fields in mask order: byte ascending, and
//! within a byte the high bit first. A [`FieldSet`] knows that order, so a
//! reply decodes without the caller naming each field again.
//!
//! Retired, unused and reserved bits are not in the tables, because the server
//! answers 505 when they are set.
//!
//! The ANIME command has its own, seven byte mask, in [`crate::anime`].

use std::collections::BTreeMap;

use crate::error::Error;
use crate::fields;

pub use crate::types::fields::*;
pub use crate::types::mask::*;

/// Decodes the values of one mask's fields, in reply order. Extra trailing
/// values are ignored, because the spec says new fields get added at the end.
pub(crate) fn decode_fields<F: Field>(
    set: FieldSet<F>,
    values: &[&str],
) -> Result<BTreeMap<F, Value>, Error> {
    let specs = set.specs();
    if values.len() < specs.len() {
        return Err(Error::BadData(format!(
            "reply has {} fields, {} expected",
            values.len(),
            specs.len()
        )));
    }
    specs
        .iter()
        .zip(values.iter().copied())
        .map(|(spec, raw)| Ok((spec.field, decode_value(spec.name, spec.kind, raw)?)))
        .collect()
}

fn decode_value(name: &str, kind: Kind, raw: &str) -> Result<Value, Error> {
    Ok(match kind {
        Kind::Int => Value::Int(fields::int(name, raw)?),
        Kind::Str => Value::Str(fields::line_breaks(raw)),
        Kind::Bool => Value::Bool(fields::boolean(name, raw)?),
        Kind::IntList(sep) => Value::Ints(fields::int_list(name, raw, sep)?),
        Kind::StrList(sep) => Value::Strs(fields::text_list(raw, sep)),
        Kind::DateFlags => Value::DateFlags(fields::dateflags(name, raw)?),
    })
}

#[cfg(test)]
mod tests;
