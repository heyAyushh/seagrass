use {
    crate::range::range_from_span,
    std::collections::HashMap,
    syn::{Attribute, ImplItem, ItemImpl, Path, Type},
    tower_lsp::lsp_types::Range,
};

const INIT_SPACE_ASSOCIATED_CONST: &str = "INIT_SPACE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociatedValueRange {
    pub name: String,
    pub range: Range,
    pub kind: AssociatedValueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociatedValueKind {
    Constant,
    Function,
}

pub fn collect_from_impl(
    item_impl: &ItemImpl,
    items_by_type: &mut HashMap<String, Vec<AssociatedValueRange>>,
) {
    let Some(type_name) = impl_self_type_name(item_impl) else {
        return;
    };
    let items = items_by_type.entry(type_name).or_default();
    for item in &item_impl.items {
        match item {
            ImplItem::Const(item_const) => items.push(AssociatedValueRange {
                name: item_const.ident.to_string(),
                range: range_from_span(item_const.ident.span()),
                kind: AssociatedValueKind::Constant,
            }),
            ImplItem::Fn(item_fn) => items.push(AssociatedValueRange {
                name: item_fn.sig.ident.to_string(),
                range: range_from_span(item_fn.sig.ident.span()),
                kind: AssociatedValueKind::Function,
            }),
            _ => {}
        }
    }
}

pub fn derives_init_space(attrs: &[Attribute]) -> bool {
    derives_named(attrs, "InitSpace")
}

pub fn is_generated_init_space_value(value_name: &str) -> bool {
    value_name == INIT_SPACE_ASSOCIATED_CONST
}

fn impl_self_type_name(item_impl: &ItemImpl) -> Option<String> {
    let Type::Path(self_ty) = item_impl.self_ty.as_ref() else {
        return None;
    };
    self_ty
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn derives_named(attrs: &[Attribute], name: &str) -> bool {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .any(|attr| {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<Path, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|paths| paths.iter().any(|path| path.is_ident(name)))
        })
}
