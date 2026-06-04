//! Collects Anchor `#[error_code]` enums and their variants so constraint error
//! annotations (`... @ MyError::Variant`) can be completed.

use {super::has_attr, syn::ItemEnum};

const ERROR_CODE_ATTRIBUTE: &str = "error_code";

#[derive(Debug, Clone)]
pub struct ErrorCodeEnum {
    pub name: String,
    pub variants: Vec<String>,
}

pub(super) fn error_code_enum(item_enum: &ItemEnum) -> Option<ErrorCodeEnum> {
    if !has_attr(&item_enum.attrs, ERROR_CODE_ATTRIBUTE) {
        return None;
    }
    Some(ErrorCodeEnum {
        name: item_enum.ident.to_string(),
        variants: item_enum
            .variants
            .iter()
            .map(|variant| variant.ident.to_string())
            .collect(),
    })
}
