use syn::parse::Parser;

// Only assertion-style macros here: parsing every macro would false-positive on
// format-style inputs such as `msg!("value={value}")`.
const RUNTIME_ASSERTION_MACROS: &[&str] = &[
    "assert",
    "assert_eq",
    "assert_ne",
    "require",
    "require_eq",
    "require_gt",
    "require_gte",
    "require_keys_eq",
    "require_keys_neq",
    "require_neq",
];

const DEBUG_ASSERTION_MACROS: &[&str] = &["debug_assert", "debug_assert_eq", "debug_assert_ne"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct DelimiterFrame {
    close: char,
    macro_name: Option<String>,
}

pub(crate) fn assertion_macro_arguments(node: &syn::Macro) -> Vec<syn::Expr> {
    if !is_expression_assertion_macro(node) {
        return Vec::new();
    }
    parse_expression_arguments(node)
}

pub(crate) fn runtime_assertion_macro_arguments(node: &syn::Macro) -> Vec<syn::Expr> {
    if !is_runtime_assertion_macro(node) {
        return Vec::new();
    }
    parse_expression_arguments(node)
}

pub(crate) fn has_open_expression_assertion_macro(source: &str, offset: usize) -> bool {
    let Some(prefix) = source.get(..offset.min(source.len())) else {
        return false;
    };
    let mut stack = Vec::<DelimiterFrame>::new();
    let mut scanner = CodeScanner::default();
    for (index, ch) in prefix.char_indices() {
        if !scanner.accept(ch) {
            continue;
        }
        match ch {
            '(' | '[' | '{' => stack.push(DelimiterFrame {
                close: matching_close(ch),
                macro_name: macro_name_before_delimiter(prefix, index),
            }),
            ')' | ']' | '}' if stack.last().is_some_and(|frame| frame.close == ch) => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.iter().any(|frame| {
        frame
            .macro_name
            .as_deref()
            .is_some_and(is_expression_assertion_macro_name)
    })
}

fn parse_expression_arguments(node: &syn::Macro) -> Vec<syn::Expr> {
    syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        .parse2(node.tokens.clone())
        .map(|expressions| expressions.into_iter().collect())
        .unwrap_or_default()
}

fn is_expression_assertion_macro(node: &syn::Macro) -> bool {
    node.path
        .segments
        .last()
        .is_some_and(|segment| is_expression_assertion_macro_name(&segment.ident.to_string()))
}

fn is_runtime_assertion_macro(node: &syn::Macro) -> bool {
    node.path
        .segments
        .last()
        .is_some_and(|segment| is_runtime_assertion_macro_name(&segment.ident.to_string()))
}

fn is_expression_assertion_macro_name(macro_name: &str) -> bool {
    is_runtime_assertion_macro_name(macro_name) || DEBUG_ASSERTION_MACROS.contains(&macro_name)
}

fn is_runtime_assertion_macro_name(macro_name: &str) -> bool {
    RUNTIME_ASSERTION_MACROS.contains(&macro_name)
}

fn macro_name_before_delimiter(source: &str, open_index: usize) -> Option<String> {
    let before_open = source.get(..open_index)?.trim_end();
    let before_bang = before_open.strip_suffix('!')?.trim_end();
    let start = before_bang
        .char_indices()
        .rev()
        .find_map(|(index, ch)| {
            (!(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
                .then_some(index + ch.len_utf8())
        })
        .unwrap_or(0);
    before_bang
        .get(start..)?
        .rsplit("::")
        .next()
        .filter(|name| crate::syntax::is_ascii_identifier(name))
        .map(str::to_string)
}

fn matching_close(open: char) -> char {
    match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => open,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CodeScanner {
    #[default]
    Code,
    Slash,
    LineComment,
    BlockComment {
        previous: char,
    },
    String {
        escaped: bool,
    },
}

impl CodeScanner {
    fn accept(&mut self, ch: char) -> bool {
        match (*self, ch) {
            (Self::Code, '/') => {
                *self = Self::Slash;
                false
            }
            (Self::Slash, '/') => {
                *self = Self::LineComment;
                false
            }
            (Self::Slash, '*') => {
                *self = Self::BlockComment { previous: '\0' };
                false
            }
            (Self::Slash, _) => {
                *self = Self::Code;
                true
            }
            (Self::Code, '"') => {
                *self = Self::String { escaped: false };
                false
            }
            (Self::LineComment, '\n') => {
                *self = Self::Code;
                false
            }
            (Self::LineComment, _) => false,
            (Self::BlockComment { previous: '*' }, '/') => {
                *self = Self::Code;
                false
            }
            (Self::BlockComment { .. }, _) => {
                *self = Self::BlockComment { previous: ch };
                false
            }
            (Self::String { escaped: true }, _) => {
                *self = Self::String { escaped: false };
                false
            }
            (Self::String { escaped: false }, '\\') => {
                *self = Self::String { escaped: true };
                false
            }
            (Self::String { escaped: false }, '"') => {
                *self = Self::Code;
                false
            }
            (Self::String { escaped: false }, _) => false,
            (Self::Code, _) => true,
        }
    }
}
