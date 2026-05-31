use {
    super::TypedLocalValue,
    crate::{document::ParsedDocument, lsp::scope::TextHandlerBinding, workspace::WorkspaceIndex},
};

pub(super) fn typed_value_from_binding(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    visible_values: &[TypedLocalValue],
    visible_bindings: &[TextHandlerBinding],
    binding: &TextHandlerBinding,
) -> Option<TypedLocalValue> {
    let type_name = binding
        .type_display
        .as_deref()
        .and_then(type_name_from_text)
        .or_else(|| {
            binding.initializer_text.as_deref().and_then(|initializer| {
                let expr = syn::parse_str::<syn::Expr>(initializer).ok()?;
                super::text_context_account_type_name(
                    document,
                    workspace_index,
                    visible_bindings,
                    &expr,
                )
            })
        })
        .or_else(|| {
            binding
                .initializer_text
                .as_deref()
                .and_then(text_constructed_type_name)
        })
        .or_else(|| {
            binding.initializer_text.as_deref().and_then(|initializer| {
                text_inferred_type_name(
                    document,
                    workspace_index,
                    visible_values,
                    visible_bindings,
                    initializer,
                )
            })
        })?;

    Some(TypedLocalValue {
        name: binding.name.clone(),
        type_name,
    })
}

fn type_name_from_text(text: &str) -> Option<String> {
    let ty = syn::parse_str::<syn::Type>(text).ok()?;
    super::local_value_type_name_from_type(&ty)
}

fn text_constructed_type_name(initializer: &str) -> Option<String> {
    initializer
        .split_once('{')
        .map(|(head, _)| head.trim())
        .filter(|head| is_identifier_path(head))?
        .rsplit("::")
        .next()
        .map(str::to_string)
}

fn text_inferred_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    visible_values: &[TypedLocalValue],
    visible_bindings: &[TextHandlerBinding],
    initializer: &str,
) -> Option<String> {
    let expr = syn::parse_str::<syn::Expr>(initializer).ok()?;
    super::expression_type_name_with_context_scope(
        document,
        workspace_index,
        &expr,
        &|name| {
            visible_values
                .iter()
                .rev()
                .find(|value| value.name == name)
                .map(|value| value.type_name.clone())
        },
        &|name| {
            visible_bindings
                .iter()
                .rev()
                .find(|binding| binding.name == name)
                .and_then(context_type_name_from_binding)
        },
    )
}

fn context_type_name_from_binding(binding: &TextHandlerBinding) -> Option<String> {
    binding
        .type_display
        .as_deref()
        .and_then(super::context_type_name_from_text)
}

fn is_identifier_path(value: &str) -> bool {
    !value.is_empty() && value.split("::").all(is_identifier)
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
