use proptest::{prelude::*, string::string_regex};

const RUST_IDENTIFIER_PATTERN: &str = "[a-z][a-z0-9_]{0,10}";
const RUST_TYPE_IDENTIFIER_PATTERN: &str = "[A-Z][A-Za-z0-9_]{1,10}";
const RESERVED_RUST_IDENTIFIERS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];
const RESERVED_RUST_TYPE_IDENTIFIERS: &[&str] = &["Self"];

pub(crate) fn rust_identifier() -> impl Strategy<Value = String> {
    string_regex(RUST_IDENTIFIER_PATTERN)
        .expect("Rust identifier regex should compile")
        .prop_filter("generated identifier must not be a Rust keyword", |value| {
            !RESERVED_RUST_IDENTIFIERS.contains(&value.as_str())
        })
}

pub(crate) fn rust_type_identifier() -> impl Strategy<Value = String> {
    string_regex(RUST_TYPE_IDENTIFIER_PATTERN)
        .expect("Rust type identifier regex should compile")
        .prop_filter(
            "generated type identifier must not be a Rust keyword",
            |value| !RESERVED_RUST_TYPE_IDENTIFIERS.contains(&value.as_str()),
        )
}
