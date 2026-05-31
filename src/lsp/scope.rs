use {
    crate::range::byte_offset_at,
    syn::parse::Parser,
    syn::{BinOp, Expr, GenericArgument, Pat, PathArguments, Type},
    tower_lsp::lsp_types::Position,
};

const FUNCTION_CONTEXT_NEEDLE: &str = "Context<";

pub(crate) fn collect_pattern_bindings(pat: &Pat, names: &mut Vec<String>) {
    match pat {
        Pat::Ident(ident) => names.push(ident.ident.to_string()),
        Pat::Reference(reference) => collect_pattern_bindings(&reference.pat, names),
        Pat::Slice(slice) => {
            for element in &slice.elems {
                collect_pattern_bindings(element, names);
            }
        }
        Pat::Struct(strukt) => {
            for field in &strukt.fields {
                collect_pattern_bindings(&field.pat, names);
            }
        }
        Pat::Tuple(tuple) => {
            for element in &tuple.elems {
                collect_pattern_bindings(element, names);
            }
        }
        Pat::TupleStruct(tuple) => {
            for element in &tuple.elems {
                collect_pattern_bindings(element, names);
            }
        }
        Pat::Type(typed) => collect_pattern_bindings(&typed.pat, names),
        Pat::Or(or) => {
            for case in &or.cases {
                collect_pattern_bindings(case, names);
            }
        }
        Pat::Paren(paren) => collect_pattern_bindings(&paren.pat, names),
        _ => {}
    }
}

pub(crate) fn collect_condition_pattern_bindings(expr: &Expr, names: &mut Vec<String>) {
    match expr {
        Expr::Let(expr_let) => collect_pattern_bindings(&expr_let.pat, names),
        Expr::Binary(binary) if matches!(binary.op, BinOp::And(_)) => {
            collect_condition_pattern_bindings(&binary.left, names);
            collect_condition_pattern_bindings(&binary.right, names);
        }
        Expr::Group(group) => collect_condition_pattern_bindings(&group.expr, names),
        Expr::Paren(paren) => collect_condition_pattern_bindings(&paren.expr, names),
        _ => {}
    }
}

pub(crate) fn pattern_binding_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(ident) => Some(ident.ident.to_string()),
        Pat::Reference(reference) => pattern_binding_name(&reference.pat),
        Pat::Type(typed) => pattern_binding_name(&typed.pat),
        Pat::Paren(paren) => pattern_binding_name(&paren.pat),
        _ => None,
    }
}

pub(crate) fn item_fn_has_anchor_context_arg(item_fn: &syn::ItemFn) -> bool {
    item_fn.sig.inputs.iter().any(|input| match input {
        syn::FnArg::Typed(pat_type) => type_has_anchor_context_arg(pat_type.ty.as_ref()),
        syn::FnArg::Receiver(_) => false,
    })
}

pub(crate) fn last_function_keyword_before(source: &str) -> Option<usize> {
    source
        .rmatch_indices("fn")
        .find_map(|(idx, _)| function_keyword_at(source, idx).then_some(idx))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextHandlerBinding {
    pub name: String,
    pub type_display: Option<String>,
    pub initializer_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextHandlerScope {
    has_anchor_context: bool,
    bindings: Vec<TextHandlerBinding>,
}

impl TextHandlerScope {
    pub(crate) fn at_position(source: &str, position: Position) -> Option<Self> {
        let offset = byte_offset_at(source, position)?;
        Self::before_offset(source, offset)
    }

    pub(crate) fn before_offset(source: &str, offset: usize) -> Option<Self> {
        let before = source.get(..offset.min(source.len()))?;
        let function_start = last_function_keyword_before(before)?;
        let function_prefix = before.get(function_start..)?;
        let (signature, body_prefix) = function_prefix.split_once('{')?;
        let has_anchor_context = signature.contains(FUNCTION_CONTEXT_NEEDLE);
        let mut bindings = text_function_input_bindings(signature);
        bindings.extend(text_active_pattern_bindings(completed_body_lines(
            body_prefix,
        )));
        bindings.extend(text_local_binding_bindings(completed_body_lines(
            body_prefix,
        )));
        Some(Self {
            has_anchor_context,
            bindings,
        })
    }

    pub(crate) const fn has_anchor_context(&self) -> bool {
        self.has_anchor_context
    }

    pub(crate) fn bindings(&self) -> &[TextHandlerBinding] {
        &self.bindings
    }
}

pub(crate) fn type_has_anchor_context_arg(ty: &Type) -> bool {
    let ty = match ty {
        Type::Reference(reference) => reference.elem.as_ref(),
        _ => ty,
    };
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path.path.segments.iter().any(|segment| {
        (segment.ident == "Context" || segment.ident.to_string().ends_with("Context"))
            && matches!(&segment.arguments, PathArguments::AngleBracketed(_))
            && context_type_has_account_argument(&segment.arguments)
    })
}

pub(crate) fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

fn completed_body_lines(body_prefix: &str) -> &str {
    body_prefix
        .rsplit_once('\n')
        .map_or("", |(completed_lines, _)| completed_lines)
}

fn function_keyword_at(source: &str, idx: usize) -> bool {
    let end = idx + "fn".len();
    let has_leading_boundary = source[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_identifier_char(ch));
    let has_trailing_whitespace = source[end..]
        .chars()
        .next()
        .is_some_and(char::is_whitespace);

    has_leading_boundary && has_trailing_whitespace
}

fn text_function_input_bindings(signature: &str) -> Vec<TextHandlerBinding> {
    let Some(open) = signature.find('(') else {
        return Vec::new();
    };
    let Some(close) = matching_close_delimiter(signature, open, '(', ')') else {
        return Vec::new();
    };
    split_top_level_commas(&signature[open + '('.len_utf8()..close])
        .into_iter()
        .filter_map(text_function_input_binding)
        .collect()
}

fn text_function_input_binding(input: &str) -> Option<TextHandlerBinding> {
    let (name, ty) = input.trim().split_once(':')?;
    let name = name.trim().strip_prefix("mut ").unwrap_or(name.trim());
    is_identifier(name).then(|| TextHandlerBinding {
        name: name.to_string(),
        type_display: (!ty.trim().is_empty()).then(|| ty.trim().to_string()),
        initializer_text: None,
    })
}

fn text_local_binding_bindings(body_prefix: &str) -> Vec<TextHandlerBinding> {
    let mut depth = 0usize;
    let mut bindings = Vec::new();
    for line in body_prefix.lines() {
        if depth == 0 {
            bindings.extend(text_local_binding_binding(line));
        }
        depth = line.chars().fold(depth, |depth, ch| match ch {
            '{' => depth + 1,
            '}' => depth.saturating_sub(1),
            _ => depth,
        });
    }
    bindings
}

fn text_local_binding_binding(line: &str) -> Option<TextHandlerBinding> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("let ")?;
    let (left, right) = rest.split_once('=')?;
    let left = left.trim().strip_prefix("mut ").unwrap_or(left.trim());
    let (name, ty) = left
        .split_once(':')
        .map_or((left, None), |(name, ty)| (name.trim(), Some(ty.trim())));
    is_identifier(name).then(|| TextHandlerBinding {
        name: name.to_string(),
        type_display: ty.filter(|ty| !ty.is_empty()).map(str::to_string),
        initializer_text: trimmed_initializer_text(right),
    })
}

fn text_active_pattern_bindings(body_prefix: &str) -> Vec<TextHandlerBinding> {
    let mut depth = 0usize;
    let mut frames = Vec::<TextPatternFrame>::new();
    for line in body_prefix.lines() {
        let pattern_bindings = text_line_pattern_bindings(line);
        let mut pending_bindings = (!pattern_bindings.is_empty()).then_some(pattern_bindings);

        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    if let Some(bindings) = pending_bindings.take() {
                        frames.push(TextPatternFrame { depth, bindings });
                    }
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    frames.retain(|frame| frame.depth <= depth);
                }
                _ => {}
            }
        }
    }

    frames
        .into_iter()
        .flat_map(|frame| frame.bindings)
        .collect()
}

struct TextPatternFrame {
    depth: usize,
    bindings: Vec<TextHandlerBinding>,
}

fn text_line_pattern_bindings(line: &str) -> Vec<TextHandlerBinding> {
    text_if_or_while_let_pattern(line)
        .or_else(|| text_match_arm_pattern(line))
        .and_then(text_pattern_bindings)
        .unwrap_or_default()
}

fn text_if_or_while_let_pattern(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let pattern = trimmed
        .strip_prefix("if let ")
        .or_else(|| trimmed.strip_prefix("while let "))?;
    pattern.split_once('=').map(|(pattern, _)| pattern.trim())
}

fn text_match_arm_pattern(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let (pattern, _) = trimmed.split_once("=>")?;
    Some(pattern.trim())
}

fn text_pattern_bindings(pattern: &str) -> Option<Vec<TextHandlerBinding>> {
    let pattern = Pat::parse_single.parse_str(pattern).ok()?;
    let mut names = Vec::new();
    collect_pattern_bindings(&pattern, &mut names);
    Some(
        names
            .into_iter()
            .map(|name| TextHandlerBinding {
                name,
                type_display: None,
                initializer_text: None,
            })
            .collect(),
    )
}

fn trimmed_initializer_text(text: &str) -> Option<String> {
    let initializer = text.trim().trim_end_matches(';').trim();
    (!initializer.is_empty()).then(|| initializer.to_string())
}

fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(text[start..idx].trim());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(text[start..].trim());
    parts
}

fn matching_close_delimiter(
    source: &str,
    open: usize,
    open_char: char,
    close_char: char,
) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in source.get(open..)?.char_indices() {
        let absolute = open + idx;
        if ch == open_char {
            depth += 1;
        } else if ch == close_char {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(absolute);
            }
        }
    }
    None
}

fn context_type_has_account_argument(arguments: &PathArguments) -> bool {
    let PathArguments::AngleBracketed(args) = arguments else {
        return false;
    };
    args.args
        .iter()
        .any(|arg| matches!(arg, GenericArgument::Type(Type::Path(_))))
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(is_identifier_char)
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
