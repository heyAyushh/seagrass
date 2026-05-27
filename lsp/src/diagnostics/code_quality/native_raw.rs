use {
    crate::{
        diagnostics::{
            diagnostic_from_span,
            lint::{run_lint_visitor_on_functions, Applicability, Confidence, LintVisitor, Region},
            registry::AnchorDiagnosticKind,
        },
        document::ParsedDocument,
    },
    quote::ToTokens,
    std::collections::{HashMap, HashSet},
    syn::visit::{self, Visit},
    tower_lsp::lsp_types::Diagnostic,
};

use super::ProgramKind;

const RAW_ACCOUNT_DATA_METHODS: &[&str] = &["try_borrow_data", "try_borrow_mut_data"];
const RAW_ACCOUNT_DATA_FIELD_METHODS: &[&str] = &["borrow", "borrow_mut"];
const RAW_DESERIALIZATION_CALLS: &[&str] =
    &["try_from_slice", "deserialize", "try_deserialize_unchecked"];
const OWNER_VALIDATION_HELPERS: &[&str] = &[
    "assert_owner",
    "check_owner",
    "validate_owner",
    "require_owner",
];
const TYPE_VALIDATION_HELPERS: &[&str] = &[
    "assert_type",
    "check_type",
    "validate_type",
    "assert_account_type",
    "check_account_type",
    "assert_discriminator",
    "check_discriminator",
    "validate_discriminator",
];

pub(super) fn diagnostics(document: &ParsedDocument) -> Vec<Diagnostic> {
    let program_kind = super::program_kind(document);
    if !matches!(
        program_kind,
        ProgramKind::NativeSolana | ProgramKind::Pinocchio
    ) || document.syntax().items.is_empty()
    {
        return Vec::new();
    }

    run_lint_visitor_on_functions(document, |item_fn| {
        NativeRawAccountInvariantVisitor::new(program_kind, item_fn)
    })
}

#[derive(Debug)]
struct NativeRawDataRead {
    span: proc_macro2::Span,
    receiver: Option<String>,
    account_index: Option<usize>,
}

struct NativeRawAccountInvariantVisitor {
    program_kind: ProgramKind,
    account_values: HashSet<String>,
    account_collections: HashSet<String>,
    account_origins: HashMap<String, AccountOrigin>,
    raw_data_reads: Vec<NativeRawDataRead>,
    raw_deserializations: Vec<proc_macro2::Span>,
    has_owner_validation: bool,
    has_discriminator_validation: bool,
}

impl NativeRawAccountInvariantVisitor {
    fn new(program_kind: ProgramKind, item_fn: &syn::ItemFn) -> Self {
        let mut visitor = Self {
            program_kind,
            account_values: HashSet::new(),
            account_collections: HashSet::new(),
            account_origins: HashMap::new(),
            raw_data_reads: Vec::new(),
            raw_deserializations: Vec::new(),
            has_owner_validation: false,
            has_discriminator_validation: false,
        };
        item_fn
            .sig
            .inputs
            .iter()
            .filter_map(account_binding_from_fn_arg)
            .for_each(|binding| visitor.register_account_binding(binding));
        visitor
    }

    fn register_account_binding(&mut self, binding: AccountBinding) {
        match binding.kind {
            AccountBindingKind::Value => {
                self.account_values.insert(binding.name);
            }
            AccountBindingKind::Collection => {
                self.account_collections.insert(binding.name);
            }
        }
    }

    fn finish(self) -> Vec<Diagnostic> {
        let Some(read) = self.raw_data_reads.first() else {
            return Vec::new();
        };

        let mut diagnostics = Vec::new();
        if !self.has_owner_validation {
            diagnostics.push(native_owner_validation_diagnostic(self.program_kind, read));
        }
        if !self.raw_deserializations.is_empty() && !self.has_discriminator_validation {
            diagnostics.push(native_discriminator_validation_diagnostic(
                self.program_kind,
                read,
                self.raw_deserializations[0],
            ));
        }
        diagnostics
    }

    fn record_local_account_origin(&mut self, node: &syn::Local) {
        let Some(local_name) = local_ident_name(node) else {
            return;
        };
        let Some(init) = node.init.as_ref() else {
            return;
        };
        if let Some(origin) = self.account_collection_origin(&init.expr) {
            self.account_values.insert(local_name.clone());
            self.account_origins.insert(local_name, origin);
        }
    }

    fn record_raw_data_read(&mut self, receiver: &syn::Expr, span: proc_macro2::Span) {
        let receiver_text = normalized_token_text(receiver);
        if !self.is_account_value_expression(receiver, &receiver_text) {
            return;
        }
        self.raw_data_reads.push(NativeRawDataRead {
            account_index: self.account_index_for_receiver(receiver, &receiver_text),
            receiver: Some(receiver_text),
            span,
        });
    }

    fn record_data_field_read(&mut self, receiver: &syn::Expr, span: proc_macro2::Span) {
        let Some(account_expr) = data_field_account_expr(receiver) else {
            return;
        };
        self.record_raw_data_read(account_expr, span);
    }

    fn is_account_value_expression(&self, expr: &syn::Expr, text: &str) -> bool {
        self.account_values.contains(text) || self.account_collection_origin(expr).is_some()
    }

    fn account_collection_origin(&self, expr: &syn::Expr) -> Option<AccountOrigin> {
        account_collection_access(expr).and_then(|access| {
            self.account_collections
                .contains(&access.collection)
                .then_some(AccountOrigin {
                    account_index: access.index,
                })
        })
    }

    fn account_index_for_receiver(
        &self,
        receiver: &syn::Expr,
        receiver_text: &str,
    ) -> Option<usize> {
        self.account_collection_origin(receiver)
            .and_then(|origin| origin.account_index)
            .or_else(|| {
                self.account_origins
                    .get(receiver_text)
                    .and_then(|origin| origin.account_index)
            })
    }
}

impl<'ast> LintVisitor<'ast> for NativeRawAccountInvariantVisitor {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/security.owner-check";

    fn finish(self) -> Vec<Diagnostic> {
        NativeRawAccountInvariantVisitor::finish(self)
    }
}

impl<'ast> Visit<'ast> for NativeRawAccountInvariantVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_local(&mut self, node: &'ast syn::Local) {
        self.record_local_account_origin(node);
        visit::visit_local(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if RAW_ACCOUNT_DATA_METHODS.contains(&method.as_str()) {
            self.record_raw_data_read(&node.receiver, node.method.span());
        }
        if RAW_ACCOUNT_DATA_FIELD_METHODS.contains(&method.as_str()) {
            self.record_data_field_read(&node.receiver, node.method.span());
        }
        if RAW_DESERIALIZATION_CALLS.contains(&method.as_str()) {
            self.raw_deserializations.push(node.method.span());
        }
        if method == "owner" {
            self.has_owner_validation = true;
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Some(ident) = called_function_ident(&node.func) {
            if ident_matches_any(ident, RAW_DESERIALIZATION_CALLS) {
                self.raw_deserializations.push(ident.span());
            }
            if ident_matches_any(ident, OWNER_VALIDATION_HELPERS) {
                self.has_owner_validation = true;
            }
            if ident_matches_any(ident, TYPE_VALIDATION_HELPERS) {
                self.has_discriminator_validation = true;
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if member_is_named(&node.member, "owner") {
            self.has_owner_validation = true;
        }
        visit::visit_expr_field(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node
            .path
            .segments
            .iter()
            .any(|segment| segment.ident == "DISCRIMINATOR")
        {
            self.has_discriminator_validation = true;
        }
        visit::visit_expr_path(self, node);
    }
}

fn native_owner_validation_diagnostic(
    program_kind: ProgramKind,
    read: &NativeRawDataRead,
) -> Diagnostic {
    diagnostic_from_span(
        read.span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Raw account data is read without visible owner validation.".to_string(),
        Some(serde_json::json!({
            "quickfix": "add-owner-check",
            "attack": "owner-checks",
            "topic": "seagrass/security.owner-check",
            "corpusMode": "legacy-invariant",
            "evidenceSource": "native-account-data-scan",
            "accountIndex": read.account_index,
            "accountExpression": read.receiver,
            "configKey": "security.ownerChecks",
            "strictNative": true,
            "programKind": program_kind.as_str(),
            "suggestion": "Check the account owner before interpreting raw account data.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    )
}

fn native_discriminator_validation_diagnostic(
    program_kind: ProgramKind,
    read: &NativeRawDataRead,
    span: proc_macro2::Span,
) -> Diagnostic {
    diagnostic_from_span(
        span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Raw account data is deserialized without visible discriminator/type validation."
            .to_string(),
        Some(serde_json::json!({
            "quickfix": "add-discriminator-check",
            "attack": "type-cosplay",
            "topic": "seagrass/security.type-cosplay",
            "corpusMode": "legacy-invariant",
            "evidenceSource": "native-account-data-scan",
            "accountIndex": read.account_index,
            "accountExpression": read.receiver,
            "configKey": "security.typeCosplay",
            "strictNative": true,
            "programKind": program_kind.as_str(),
            "suggestion": "Validate discriminator/type bytes before decoding raw account data.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    )
}

#[derive(Debug)]
struct AccountBinding {
    name: String,
    kind: AccountBindingKind,
}

#[derive(Debug)]
enum AccountBindingKind {
    Value,
    Collection,
}

#[derive(Debug, Clone)]
struct AccountOrigin {
    account_index: Option<usize>,
}

#[derive(Debug)]
struct AccountCollectionAccess {
    collection: String,
    index: Option<usize>,
}

fn account_binding_from_fn_arg(arg: &syn::FnArg) -> Option<AccountBinding> {
    let syn::FnArg::Typed(pat_type) = arg else {
        return None;
    };
    let name = pat_ident_name(&pat_type.pat)?;
    account_binding_kind(&pat_type.ty).map(|kind| AccountBinding { name, kind })
}

fn account_binding_kind(ty: &syn::Type) -> Option<AccountBindingKind> {
    if is_account_info_collection_type(ty) {
        Some(AccountBindingKind::Collection)
    } else if is_account_info_value_type(ty) {
        Some(AccountBindingKind::Value)
    } else {
        None
    }
}

fn is_account_info_collection_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Reference(reference) => is_account_info_collection_type(&reference.elem),
        syn::Type::Slice(slice) => is_account_info_value_type(&slice.elem),
        syn::Type::Array(array) => is_account_info_value_type(&array.elem),
        syn::Type::Path(type_path) => {
            let Some(segment) = type_path.path.segments.last() else {
                return false;
            };
            segment.ident == "Vec" && generic_type_arg_is_account_info(segment)
        }
        _ => false,
    }
}

fn is_account_info_value_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Reference(reference) => is_account_info_value_type(&reference.elem),
        syn::Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "AccountInfo"),
        _ => false,
    }
}

fn generic_type_arg_is_account_info(segment: &syn::PathSegment) -> bool {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return false;
    };
    arguments.args.iter().any(|argument| {
        matches!(argument, syn::GenericArgument::Type(ty) if is_account_info_value_type(ty))
    })
}

fn local_ident_name(local: &syn::Local) -> Option<String> {
    pat_ident_name(&local.pat)
}

fn pat_ident_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_string()),
        syn::Pat::Reference(reference) => pat_ident_name(&reference.pat),
        syn::Pat::Type(pat_type) => pat_ident_name(&pat_type.pat),
        _ => None,
    }
}

fn data_field_account_expr(receiver: &syn::Expr) -> Option<&syn::Expr> {
    let syn::Expr::Field(field) = receiver else {
        return None;
    };
    member_is_named(&field.member, "data").then_some(field.base.as_ref())
}

fn account_collection_access(expr: &syn::Expr) -> Option<AccountCollectionAccess> {
    match expr {
        syn::Expr::Index(index) => {
            let collection = path_ident_name(&index.expr)?;
            Some(AccountCollectionAccess {
                collection,
                index: unsigned_literal(&index.index),
            })
        }
        syn::Expr::MethodCall(method_call) if method_call.method == "get" => {
            let collection = path_ident_name(&method_call.receiver)?;
            Some(AccountCollectionAccess {
                collection,
                index: method_call.args.first().and_then(unsigned_literal),
            })
        }
        syn::Expr::Group(group) => account_collection_access(&group.expr),
        syn::Expr::Paren(paren) => account_collection_access(&paren.expr),
        syn::Expr::Reference(reference) => account_collection_access(&reference.expr),
        _ => None,
    }
}

fn path_ident_name(expr: &syn::Expr) -> Option<String> {
    let syn::Expr::Path(path) = expr else {
        return None;
    };
    path.path.get_ident().map(ToString::to_string)
}

fn unsigned_literal(expr: &syn::Expr) -> Option<usize> {
    match expr {
        syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
            syn::Lit::Int(lit) => lit.base10_parse().ok(),
            _ => None,
        },
        syn::Expr::Group(group) => unsigned_literal(&group.expr),
        syn::Expr::Paren(paren) => unsigned_literal(&paren.expr),
        _ => None,
    }
}

fn called_function_ident(func: &syn::Expr) -> Option<&syn::Ident> {
    let syn::Expr::Path(expr_path) = func else {
        return None;
    };
    expr_path.path.segments.last().map(|segment| &segment.ident)
}

fn ident_matches_any(ident: &syn::Ident, candidates: &[&str]) -> bool {
    candidates.iter().any(|candidate| ident == *candidate)
}

fn member_is_named(member: &syn::Member, name: &str) -> bool {
    matches!(member, syn::Member::Named(ident) if ident == name)
}

fn normalized_token_text(tokens: &impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}
