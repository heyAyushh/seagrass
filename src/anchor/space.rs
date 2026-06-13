//! Account data size estimates for Anchor `space =` surfaces.
//!
//! Fact table verified against `anchor-derive-space-0.32.1/src/lib.rs`
//! (`len_from_type`, `derive_init_space`): primitives, `Pubkey`, arrays,
//! tuples, `Option`, `String`, `Vec`, nested types, and enum max payloads.
//! Re-check this table whenever the pinned Anchor support source changes.

use {
    crate::{
        document::{ParsedDocument, SymbolRange},
        workspace::WorkspaceIndex,
    },
    quote::ToTokens,
    std::collections::{HashSet, VecDeque},
    syn::{GenericArgument, PathArguments, Type},
};

const BOOL_BYTES: u64 = 1;
const U8_BYTES: u64 = 1;
const U16_BYTES: u64 = 2;
const U32_BYTES: u64 = 4;
const U64_BYTES: u64 = 8;
const U128_BYTES: u64 = 16;
const PUBKEY_BYTES: u64 = 32;
const BORSH_LENGTH_PREFIX_BYTES: u64 = 4;
const OPTION_TAG_BYTES: u64 = 1;
const ENUM_TAG_BYTES: u64 = 1;
const RECURSIVE_TYPE_REASON: &str = "recursive type";
const ZERO_COPY_REASON: &str = "zero-copy layout";
const UNRESOLVABLE_TYPE_REASON: &str = "unresolvable type";
const UNSUPPORTED_TYPE_REASON: &str = "unsupported type";
const OVERFLOW_REASON: &str = "space overflow";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpaceEstimate {
    Exact(u64),
    Formula {
        fixed: u64,
        symbolic: Vec<SymbolicPart>,
    },
    Unknown(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicPart {
    pub field: String,
    pub expr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSpaceReport {
    pub estimate: SpaceEstimate,
    pub fields: Vec<FieldSpaceReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSpaceReport {
    pub field: String,
    pub type_display: String,
    pub estimate: SpaceEstimate,
}

pub fn account_space(strukt: &SymbolRange, workspace: Option<&WorkspaceIndex>) -> SpaceEstimate {
    account_space_report(strukt, None, workspace).estimate
}

pub(crate) fn account_space_with_document(
    strukt: &SymbolRange,
    document: &ParsedDocument,
    workspace: Option<&WorkspaceIndex>,
) -> SpaceEstimate {
    account_space_report(strukt, Some(document), workspace).estimate
}

pub(crate) fn account_space_report(
    strukt: &SymbolRange,
    document: Option<&ParsedDocument>,
    workspace: Option<&WorkspaceIndex>,
) -> AccountSpaceReport {
    let mut resolver = SpaceResolver {
        document,
        workspace,
        visited: HashSet::new(),
    };
    resolver.account_report(strukt)
}

pub fn field_space(type_name: &str, generics: &[String]) -> SpaceEstimate {
    match primitive_bytes(type_name) {
        Some(bytes) => SpaceEstimate::Exact(bytes),
        None if type_name == "Option" && generics.len() == 1 => SpaceEstimate::Formula {
            fixed: OPTION_TAG_BYTES,
            symbolic: vec![SymbolicPart {
                field: generics[0].clone(),
                expr: format!("size of `{}`", generics[0]),
            }],
        },
        None if type_name == "String" => SpaceEstimate::Formula {
            fixed: BORSH_LENGTH_PREFIX_BYTES,
            symbolic: vec![SymbolicPart {
                field: type_name.to_string(),
                expr: "max_len".to_string(),
            }],
        },
        None => SpaceEstimate::Unknown(UNRESOLVABLE_TYPE_REASON),
    }
}

impl SpaceEstimate {
    pub fn exact_bytes(&self) -> Option<u64> {
        match self {
            Self::Exact(bytes) => Some(*bytes),
            Self::Formula { .. } | Self::Unknown(_) => None,
        }
    }

    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }
}

struct SpaceResolver<'a> {
    document: Option<&'a ParsedDocument>,
    workspace: Option<&'a WorkspaceIndex>,
    visited: HashSet<String>,
}

impl SpaceResolver<'_> {
    fn account_report(&mut self, strukt: &SymbolRange) -> AccountSpaceReport {
        if strukt.is_zero_copy {
            return AccountSpaceReport {
                estimate: SpaceEstimate::Unknown(ZERO_COPY_REASON),
                fields: Vec::new(),
            };
        }
        if !self.visited.insert(strukt.name.clone()) {
            return AccountSpaceReport {
                estimate: SpaceEstimate::Unknown(RECURSIVE_TYPE_REASON),
                fields: Vec::new(),
            };
        }

        let mut fields = Vec::with_capacity(strukt.fields.len());
        let mut total = SpaceEstimate::Exact(0);
        for field in &strukt.fields {
            let estimate = self.field_symbol_space(field);
            if matches!(estimate, SpaceEstimate::Unknown(_)) {
                self.visited.remove(&strukt.name);
                return AccountSpaceReport { estimate, fields };
            }
            total = add_estimates(total, estimate.clone());
            fields.push(FieldSpaceReport {
                field: field.name.clone(),
                type_display: field_type_display(field),
                estimate,
            });
        }

        self.visited.remove(&strukt.name);
        AccountSpaceReport {
            estimate: total,
            fields,
        }
    }

    fn field_symbol_space(&mut self, field: &SymbolRange) -> SpaceEstimate {
        let Some(type_signature) = field.type_signature.as_deref() else {
            return field
                .type_name
                .as_deref()
                .map(|type_name| field_space(type_name, &field.generic_type_names))
                .unwrap_or(SpaceEstimate::Unknown(UNSUPPORTED_TYPE_REASON));
        };
        let Ok(ty) = syn::parse_str::<Type>(type_signature) else {
            return SpaceEstimate::Unknown(UNSUPPORTED_TYPE_REASON);
        };
        let mut max_len_args = MaxLenArgs::new(field.max_len_args.clone());
        self.type_space(&ty, &field.name, &mut max_len_args)
    }

    fn type_space(
        &mut self,
        ty: &Type,
        field_name: &str,
        max_len_args: &mut MaxLenArgs,
    ) -> SpaceEstimate {
        match ty {
            Type::Array(array) => self.array_space(array, field_name, max_len_args),
            Type::Path(type_path) => {
                let Some(segment) = type_path.path.segments.last() else {
                    return SpaceEstimate::Unknown(UNSUPPORTED_TYPE_REASON);
                };
                let type_name = segment.ident.to_string();
                if let Some(bytes) = primitive_bytes(&type_name) {
                    return SpaceEstimate::Exact(bytes);
                }
                match type_name.as_str() {
                    "String" => string_space(field_name, max_len_args),
                    "Option" => self.option_space(&segment.arguments, field_name, max_len_args),
                    "Vec" => self.vec_space(&segment.arguments, field_name, max_len_args),
                    _ => self.named_type_space(&type_name),
                }
            }
            Type::Tuple(tuple) => tuple
                .elems
                .iter()
                .map(|elem| self.type_space(elem, field_name, max_len_args))
                .try_fold(SpaceEstimate::Exact(0), try_add_estimates)
                .unwrap_or(SpaceEstimate::Unknown(UNSUPPORTED_TYPE_REASON)),
            _ => SpaceEstimate::Unknown(UNSUPPORTED_TYPE_REASON),
        }
    }

    fn array_space(
        &mut self,
        array: &syn::TypeArray,
        field_name: &str,
        max_len_args: &mut MaxLenArgs,
    ) -> SpaceEstimate {
        let elem = self.type_space(&array.elem, field_name, max_len_args);
        let length_expr = normalize_tokens(&array.len);
        match literal_u64_expr(&array.len) {
            Some(length) => multiply_estimate(elem, length, field_name),
            None => match elem {
                SpaceEstimate::Exact(bytes) => SpaceEstimate::Formula {
                    fixed: 0,
                    symbolic: vec![SymbolicPart {
                        field: field_name.to_string(),
                        expr: format!("{length_expr} * {bytes}"),
                    }],
                },
                SpaceEstimate::Formula { fixed, symbolic } => SpaceEstimate::Formula {
                    fixed: 0,
                    symbolic: vec![SymbolicPart {
                        field: field_name.to_string(),
                        expr: format!("{length_expr} * ({})", formula_expr(fixed, &symbolic)),
                    }],
                },
                SpaceEstimate::Unknown(reason) => SpaceEstimate::Unknown(reason),
            },
        }
    }

    fn option_space(
        &mut self,
        arguments: &PathArguments,
        field_name: &str,
        max_len_args: &mut MaxLenArgs,
    ) -> SpaceEstimate {
        let Some(inner) = first_type_argument(arguments) else {
            return SpaceEstimate::Unknown(UNSUPPORTED_TYPE_REASON);
        };
        add_estimates(
            SpaceEstimate::Exact(OPTION_TAG_BYTES),
            self.type_space(inner, field_name, max_len_args),
        )
    }

    fn vec_space(
        &mut self,
        arguments: &PathArguments,
        field_name: &str,
        max_len_args: &mut MaxLenArgs,
    ) -> SpaceEstimate {
        let Some(inner) = first_type_argument(arguments) else {
            return SpaceEstimate::Unknown(UNSUPPORTED_TYPE_REASON);
        };
        let length = max_len_args.next();
        let item = self.type_space(inner, field_name, max_len_args);
        let payload = match length.as_deref().and_then(parse_u64) {
            Some(length) => multiply_estimate(item, length, field_name),
            None => symbolic_vec_payload(field_name, length.as_deref().unwrap_or("max_len"), item),
        };
        add_estimates(SpaceEstimate::Exact(BORSH_LENGTH_PREFIX_BYTES), payload)
    }

    fn named_type_space(&mut self, type_name: &str) -> SpaceEstimate {
        if self.visited.contains(type_name) {
            return SpaceEstimate::Unknown(RECURSIVE_TYPE_REASON);
        }
        if let Some(strukt) = self.local_struct(type_name).cloned() {
            return self.account_report(&strukt).estimate;
        }
        if let Some(enm) = self.local_enum(type_name).cloned() {
            return self.enum_space(&enm);
        }
        if let Some(strukt) = self
            .workspace
            .and_then(|workspace| workspace.account_data_struct(type_name))
            .map(|entry| entry.symbol.clone())
        {
            return self.account_report(&strukt).estimate;
        }
        SpaceEstimate::Unknown(UNRESOLVABLE_TYPE_REASON)
    }

    fn enum_space(&mut self, enm: &SymbolRange) -> SpaceEstimate {
        let mut max_variant = 0;
        for variant in &enm.variants {
            let estimate = variant
                .fields
                .iter()
                .map(|field| self.field_symbol_space(field))
                .try_fold(SpaceEstimate::Exact(0), try_add_estimates);
            let Some(bytes) = estimate.ok().and_then(|estimate| estimate.exact_bytes()) else {
                return SpaceEstimate::Unknown(UNSUPPORTED_TYPE_REASON);
            };
            max_variant = max_variant.max(bytes);
        }
        SpaceEstimate::Exact(ENUM_TAG_BYTES.saturating_add(max_variant))
    }

    fn local_struct(&self, type_name: &str) -> Option<&SymbolRange> {
        self.document?.symbols().all_structs.get(type_name)
    }

    fn local_enum(&self, type_name: &str) -> Option<&SymbolRange> {
        self.document?.symbols().enums.get(type_name)
    }
}

#[derive(Debug)]
struct MaxLenArgs {
    args: VecDeque<String>,
}

impl MaxLenArgs {
    fn new(args: Vec<String>) -> Self {
        Self { args: args.into() }
    }

    fn next(&mut self) -> Option<String> {
        self.args.pop_front()
    }
}

fn primitive_bytes(type_name: &str) -> Option<u64> {
    match type_name {
        "bool" => Some(BOOL_BYTES),
        "u8" | "i8" => Some(U8_BYTES),
        "u16" | "i16" => Some(U16_BYTES),
        "u32" | "i32" | "f32" => Some(U32_BYTES),
        "u64" | "i64" | "f64" => Some(U64_BYTES),
        "u128" | "i128" => Some(U128_BYTES),
        "Pubkey" => Some(PUBKEY_BYTES),
        _ => None,
    }
}

fn string_space(field_name: &str, max_len_args: &mut MaxLenArgs) -> SpaceEstimate {
    match max_len_args.next() {
        Some(length) => match parse_u64(&length) {
            Some(length) => SpaceEstimate::Exact(BORSH_LENGTH_PREFIX_BYTES.saturating_add(length)),
            None => SpaceEstimate::Formula {
                fixed: BORSH_LENGTH_PREFIX_BYTES,
                symbolic: vec![SymbolicPart {
                    field: field_name.to_string(),
                    expr: length,
                }],
            },
        },
        None => SpaceEstimate::Formula {
            fixed: BORSH_LENGTH_PREFIX_BYTES,
            symbolic: vec![SymbolicPart {
                field: field_name.to_string(),
                expr: "max_len".to_string(),
            }],
        },
    }
}

fn first_type_argument(arguments: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(arguments) = arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn literal_u64_expr(expr: &syn::Expr) -> Option<u64> {
    let syn::Expr::Lit(lit) = expr else {
        return None;
    };
    let syn::Lit::Int(int) = &lit.lit else {
        return None;
    };
    int.base10_parse().ok()
}

fn parse_u64(value: &str) -> Option<u64> {
    value.trim_matches(['(', ')']).parse().ok()
}

fn try_add_estimates(left: SpaceEstimate, right: SpaceEstimate) -> Result<SpaceEstimate, ()> {
    match add_estimates(left, right) {
        SpaceEstimate::Unknown(_) => Err(()),
        estimate => Ok(estimate),
    }
}

fn add_estimates(left: SpaceEstimate, right: SpaceEstimate) -> SpaceEstimate {
    match (left, right) {
        (SpaceEstimate::Unknown(reason), _) | (_, SpaceEstimate::Unknown(reason)) => {
            SpaceEstimate::Unknown(reason)
        }
        (SpaceEstimate::Exact(left), SpaceEstimate::Exact(right)) => left
            .checked_add(right)
            .map(SpaceEstimate::Exact)
            .unwrap_or(SpaceEstimate::Unknown(OVERFLOW_REASON)),
        (SpaceEstimate::Exact(left), SpaceEstimate::Formula { fixed, symbolic })
        | (SpaceEstimate::Formula { fixed, symbolic }, SpaceEstimate::Exact(left)) => left
            .checked_add(fixed)
            .map(|fixed| SpaceEstimate::Formula { fixed, symbolic })
            .unwrap_or(SpaceEstimate::Unknown(OVERFLOW_REASON)),
        (
            SpaceEstimate::Formula {
                fixed: left_fixed,
                mut symbolic,
            },
            SpaceEstimate::Formula {
                fixed: right_fixed,
                symbolic: right_symbolic,
            },
        ) => match left_fixed.checked_add(right_fixed) {
            Some(fixed) => {
                symbolic.extend(right_symbolic);
                SpaceEstimate::Formula { fixed, symbolic }
            }
            None => SpaceEstimate::Unknown(OVERFLOW_REASON),
        },
    }
}

fn multiply_estimate(estimate: SpaceEstimate, factor: u64, field_name: &str) -> SpaceEstimate {
    match estimate {
        SpaceEstimate::Exact(bytes) => bytes
            .checked_mul(factor)
            .map(SpaceEstimate::Exact)
            .unwrap_or(SpaceEstimate::Unknown(OVERFLOW_REASON)),
        SpaceEstimate::Formula { fixed, symbolic } => match fixed.checked_mul(factor) {
            Some(fixed) => SpaceEstimate::Formula {
                fixed,
                symbolic: symbolic
                    .into_iter()
                    .map(|part| SymbolicPart {
                        field: field_name.to_string(),
                        expr: format!("{factor} * ({})", part.expr),
                    })
                    .collect(),
            },
            None => SpaceEstimate::Unknown(OVERFLOW_REASON),
        },
        SpaceEstimate::Unknown(reason) => SpaceEstimate::Unknown(reason),
    }
}

fn symbolic_vec_payload(field_name: &str, length: &str, item: SpaceEstimate) -> SpaceEstimate {
    match item {
        SpaceEstimate::Exact(bytes) => SpaceEstimate::Formula {
            fixed: 0,
            symbolic: vec![SymbolicPart {
                field: field_name.to_string(),
                expr: format!("{length} * {bytes}"),
            }],
        },
        SpaceEstimate::Formula { fixed, symbolic } => SpaceEstimate::Formula {
            fixed: 0,
            symbolic: vec![SymbolicPart {
                field: field_name.to_string(),
                expr: format!("{length} * ({})", formula_expr(fixed, &symbolic)),
            }],
        },
        SpaceEstimate::Unknown(reason) => SpaceEstimate::Unknown(reason),
    }
}

pub(crate) fn formula_expr(fixed: u64, symbolic: &[SymbolicPart]) -> String {
    let mut parts = Vec::new();
    if fixed > 0 || symbolic.is_empty() {
        parts.push(fixed.to_string());
    }
    parts.extend(symbolic.iter().map(|part| part.expr.clone()));
    parts.join(" + ")
}

pub(crate) fn estimate_expr(estimate: &SpaceEstimate) -> Option<String> {
    match estimate {
        SpaceEstimate::Exact(bytes) => Some(bytes.to_string()),
        SpaceEstimate::Formula { fixed, symbolic } => Some(formula_expr(*fixed, symbolic)),
        SpaceEstimate::Unknown(_) => None,
    }
}

pub(crate) fn field_type_display(field: &SymbolRange) -> String {
    field
        .type_signature
        .clone()
        .or_else(|| {
            let type_name = field.type_name.as_ref()?;
            if field.generic_type_names.is_empty() {
                Some(type_name.clone())
            } else {
                Some(format!(
                    "{}<{}>",
                    type_name,
                    field.generic_type_names.join(", ")
                ))
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn normalize_tokens(tokens: &impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn estimate_for(source: &str, name: &str) -> SpaceEstimate {
        let document = ParsedDocument::parse(source).unwrap();
        let account = document.symbols().account_data_structs.get(name).unwrap();
        account_space_with_document(account, &document, None)
    }

    #[test]
    fn space_sizes_primitive_fact_table() {
        let source = r#"
#[account]
pub struct State {
    pub flag: bool,
    pub byte: u8,
    pub short: u16,
    pub word: u32,
    pub amount: u64,
    pub wide: u128,
    pub key: Pubkey,
}
"#;
        assert_eq!(estimate_for(source, "State"), SpaceEstimate::Exact(64));
    }

    #[test]
    fn space_sizes_option_pubkey() {
        let source = r#"
#[account]
pub struct State {
    pub authority: Option<Pubkey>,
}
"#;
        assert_eq!(estimate_for(source, "State"), SpaceEstimate::Exact(33));
    }

    #[test]
    fn space_sizes_array_with_literal_len() {
        let source = r#"
#[account]
pub struct State {
    pub slots: [u16; 4],
}
"#;
        assert_eq!(estimate_for(source, "State"), SpaceEstimate::Exact(8));
    }

    #[test]
    fn space_uses_max_len_for_string() {
        let source = r#"
#[account]
pub struct State {
    #[max_len(50)]
    pub name: String,
}
"#;
        assert_eq!(estimate_for(source, "State"), SpaceEstimate::Exact(54));
    }

    #[test]
    fn space_formula_for_string_without_max_len() {
        let source = r#"
#[account]
pub struct State {
    pub name: String,
}
"#;
        assert_eq!(
            estimate_for(source, "State"),
            SpaceEstimate::Formula {
                fixed: 4,
                symbolic: vec![SymbolicPart {
                    field: "name".to_string(),
                    expr: "max_len".to_string(),
                }],
            }
        );
    }

    #[test]
    fn space_sizes_nested_vec_max_len_args_in_order() {
        let source = r#"
#[account]
pub struct State {
    #[max_len(10, 5)]
    pub nested: Vec<Vec<u8>>,
}
"#;
        assert_eq!(estimate_for(source, "State"), SpaceEstimate::Exact(94));
    }

    #[test]
    fn space_sizes_nested_struct() {
        let source = r#"
pub struct Header {
    pub key: Pubkey,
}

#[account]
pub struct State {
    pub header: Header,
    pub count: u64,
}
"#;
        assert_eq!(estimate_for(source, "State"), SpaceEstimate::Exact(40));
    }

    #[test]
    fn space_sizes_enum_as_tag_plus_max_variant_payload() {
        let source = r#"
pub enum Choice {
    Empty,
    Full { key: Pubkey, amount: u64 },
}

#[account]
pub struct State {
    pub choice: Choice,
}
"#;
        assert_eq!(estimate_for(source, "State"), SpaceEstimate::Exact(41));
    }

    #[test]
    fn space_unknown_for_recursive_type() {
        let source = r#"
#[account]
pub struct Node {
    pub next: Node,
}
"#;
        assert_eq!(
            estimate_for(source, "Node"),
            SpaceEstimate::Unknown(RECURSIVE_TYPE_REASON)
        );
    }

    #[test]
    fn space_unknown_for_unresolvable_type() {
        let source = r#"
#[account]
pub struct State {
    pub missing: Missing,
}
"#;
        assert_eq!(
            estimate_for(source, "State"),
            SpaceEstimate::Unknown(UNRESOLVABLE_TYPE_REASON)
        );
    }

    #[test]
    fn space_unknown_for_zero_copy_account() {
        let source = r#"
#[account(zero_copy)]
pub struct State {
    pub amount: u64,
}
"#;
        assert_eq!(
            estimate_for(source, "State"),
            SpaceEstimate::Unknown(ZERO_COPY_REASON)
        );
    }
}
