pub fn is_ascii_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

pub fn is_ascii_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

pub fn is_ascii_identifier_byte(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

pub fn is_ascii_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(is_ascii_identifier_start) && chars.all(is_ascii_identifier_char)
}

pub fn is_ascii_identifier_prefix(value: &str) -> bool {
    value.chars().all(is_ascii_identifier_char)
}

pub fn is_ascii_identifier_path(value: &str, separator: &str) -> bool {
    !value.is_empty() && value.split(separator).all(is_ascii_identifier)
}

pub fn is_ascii_type_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_uppercase())
        && chars.all(is_ascii_identifier_char)
}

pub fn is_ascii_type_path(value: &str, separator: &str) -> bool {
    !value.is_empty() && value.split(separator).all(is_ascii_type_identifier)
}
