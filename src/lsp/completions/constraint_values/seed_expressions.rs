//! Maps an instruction argument's type to the seed expression that turns it
//! into PDA seed bytes. Anchor seeds must be byte slices, so each supported
//! type carries the idiomatic conversion suffix used in `seeds = [...]`.

const PDA_SEED_BYTES_DETAIL: &str = "PDA seed bytes";
const UTF8_PDA_SEED_BYTES_DETAIL: &str = "UTF-8 PDA seed bytes";
const LITTLE_ENDIAN_PDA_SEED_BYTES_DETAIL: &str = "little-endian PDA seed bytes";

const AS_REF_SUFFIX: &str = ".as_ref()";
const AS_BYTES_SUFFIX: &str = ".as_bytes()";
const LITTLE_ENDIAN_BYTES_SUFFIX: &str = ".to_le_bytes().as_ref()";
/// A `&[u8]` argument is already a byte slice, so it is used as-is.
const BORROWED_SLICE_SUFFIX: &str = "";

const BYTE_VEC_SIGNATURE: &str = "Vec<u8>";
const BORROWED_BYTE_SLICE_SIGNATURE: &str = "&[u8]";
const BYTE_ARRAY_SIGNATURE_PREFIX: &str = "[u8;";
const BORROWED_BYTE_ARRAY_SIGNATURE_PREFIX: &str = "&[u8;";
const ARRAY_SIGNATURE_SUFFIX: &str = "]";

/// Resolves the seed conversion for an argument. `type_name` is the last path
/// segment (handles scalars even behind a qualified path such as
/// `prelude::Pubkey`); `type_signature` is the normalized full type, needed to
/// tell byte containers apart from same-headed generics (e.g. `Vec<u8>` vs
/// `Vec<Pubkey>`).
pub(super) fn expression_for_argument_type(
    type_name: Option<&str>,
    type_signature: Option<&str>,
) -> Option<(&'static str, &'static str)> {
    if let Some(scalar) = type_name.and_then(scalar_seed_expression) {
        return Some(scalar);
    }
    type_signature.and_then(byte_container_seed_expression)
}

fn scalar_seed_expression(type_name: &str) -> Option<(&'static str, &'static str)> {
    match type_name {
        "Pubkey" => Some((AS_REF_SUFFIX, PDA_SEED_BYTES_DETAIL)),
        "String" => Some((AS_BYTES_SUFFIX, UTF8_PDA_SEED_BYTES_DETAIL)),
        "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128" => Some((
            LITTLE_ENDIAN_BYTES_SUFFIX,
            LITTLE_ENDIAN_PDA_SEED_BYTES_DETAIL,
        )),
        _ => None,
    }
}

fn byte_container_seed_expression(type_signature: &str) -> Option<(&'static str, &'static str)> {
    if type_signature == BYTE_VEC_SIGNATURE
        || type_signature.ends_with("::Vec<u8>")
        || is_owned_byte_array(type_signature)
    {
        return Some((AS_REF_SUFFIX, PDA_SEED_BYTES_DETAIL));
    }
    if type_signature == BORROWED_BYTE_SLICE_SIGNATURE || is_borrowed_byte_array(type_signature) {
        return Some((BORROWED_SLICE_SUFFIX, PDA_SEED_BYTES_DETAIL));
    }
    None
}

/// Matches `[u8; N]` for any const length N without binding to the size.
fn is_owned_byte_array(type_signature: &str) -> bool {
    type_signature.starts_with(BYTE_ARRAY_SIGNATURE_PREFIX)
        && type_signature.ends_with(ARRAY_SIGNATURE_SUFFIX)
}

/// Matches `&[u8; N]` for any const length N.
fn is_borrowed_byte_array(type_signature: &str) -> bool {
    type_signature.starts_with(BORROWED_BYTE_ARRAY_SIGNATURE_PREFIX)
        && type_signature.ends_with(ARRAY_SIGNATURE_SUFFIX)
}
