use proptest::{prelude::*, string::string_regex};

const RUST_IDENTIFIER_PATTERN: &str = "[a-z][a-z0-9_]{0,10}";
const RESERVED_RUST_IDENTIFIERS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

pub(super) fn rust_identifier() -> impl Strategy<Value = String> {
    string_regex(RUST_IDENTIFIER_PATTERN)
        .expect("Rust identifier regex should compile")
        .prop_filter("generated identifier must not be a Rust keyword", |value| {
            !RESERVED_RUST_IDENTIFIERS.contains(&value.as_str())
        })
}
