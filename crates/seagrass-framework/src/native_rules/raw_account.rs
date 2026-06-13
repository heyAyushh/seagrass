use {
    super::common::{
        account_iterator_collection, called_ident, compact_token_text as normalized_token_text,
        ident_matches_any, local_ident_name, member_is_named, next_account_info_iterator_name,
        pat_ident_name, unsigned_literal, AccountIteratorOrigin, INITIAL_ITERATOR_ACCOUNT_INDEX,
        TRANSPARENT_ACCOUNT_ACCESS_METHODS,
    },
    crate::{
        diagnostics::{solana_code_quality_from_span, FrameworkDocument},
        lint::{
            run_lint_visitor_on_functions, Applicability, Confidence, FunctionBody, LintVisitor,
            Region,
        },
        FrameworkKind,
    },
    std::collections::{HashMap, HashSet},
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
    },
    tower_lsp::lsp_types::Diagnostic,
};

const RAW_ACCOUNT_DATA_METHODS: &[&str] = &[
    "try_borrow_data",
    "try_borrow_mut_data",
    "borrow_data_unchecked",
];
const RAW_ACCOUNT_DATA_FIELD_METHODS: &[&str] = &["borrow", "borrow_mut"];
const RAW_DESERIALIZATION_CALLS: &[&str] = &[
    "try_from_slice",
    "deserialize",
    "try_deserialize_unchecked",
    "load",
];
const OWNER_VALIDATION_HELPERS: &[&str] = &[
    "assert_owner",
    "check_owner",
    "is_owned_by",
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
pub(super) fn diagnostics(
    document: FrameworkDocument<'_>,
    framework_kind: FrameworkKind,
) -> Vec<Diagnostic> {
    if document.syntax().items.is_empty() {
        return Vec::new();
    }

    run_lint_visitor_on_functions(document, |function| {
        NativeRawAccountInvariantVisitor::new(framework_kind, function)
    })
}

#[derive(Clone, Debug)]
struct NativeRawDataRead {
    span: proc_macro2::Span,
    receiver: Option<String>,
    account_index: Option<usize>,
}

#[derive(Debug)]
struct NativeRawDeserialization {
    span: proc_macro2::Span,
    read: Option<NativeRawDataRead>,
}

struct NativeRawAccountInvariantVisitor {
    framework_kind: FrameworkKind,
    account_values: HashSet<String>,
    account_collections: HashSet<String>,
    account_iterators: HashMap<String, AccountIteratorOrigin>,
    account_origins: HashMap<String, AccountOrigin>,
    data_origins: HashMap<String, NativeRawDataRead>,
    raw_data_reads: Vec<NativeRawDataRead>,
    raw_deserializations: Vec<NativeRawDeserialization>,
    owner_validations: AccountValidationSet,
    discriminator_validations: AccountValidationSet,
}

impl NativeRawAccountInvariantVisitor {
    fn new(framework_kind: FrameworkKind, function: FunctionBody<'_>) -> Self {
        let mut visitor = Self {
            framework_kind,
            account_values: HashSet::new(),
            account_collections: HashSet::new(),
            account_iterators: HashMap::new(),
            account_origins: HashMap::new(),
            data_origins: HashMap::new(),
            raw_data_reads: Vec::new(),
            raw_deserializations: Vec::new(),
            owner_validations: AccountValidationSet::default(),
            discriminator_validations: AccountValidationSet::default(),
        };
        function
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
        for read in &self.raw_data_reads {
            if !self.owner_validations.contains(read) {
                diagnostics.push(native_owner_validation_diagnostic(
                    self.framework_kind,
                    read,
                ));
            }
        }
        for deserialization in &self.raw_deserializations {
            let deserialized_read = deserialization.read.as_ref().unwrap_or(read);
            if !self.discriminator_validations.contains(deserialized_read) {
                diagnostics.push(native_discriminator_validation_diagnostic(
                    self.framework_kind,
                    deserialized_read,
                    deserialization.span,
                ));
            }
        }
        diagnostics
    }

    fn record_local_account_origin(&mut self, node: &syn::Local) {
        let Some(init) = node.init.as_ref() else {
            return;
        };
        if self.record_destructured_account_origins(&node.pat, &init.expr) {
            return;
        }
        let Some(local_name) = local_ident_name(node) else {
            return;
        };
        if self.record_account_iterator_origin(&local_name, &init.expr) {
            return;
        }
        if let Some(origin) = self.next_account_info_origin(&init.expr) {
            self.account_values.insert(local_name.clone());
            self.account_origins.insert(local_name, origin);
            return;
        }
        if let Some(origin) = self.account_collection_origin(&init.expr) {
            self.account_values.insert(local_name.clone());
            self.account_origins.insert(local_name, origin);
        }
    }

    fn record_account_iterator_origin(&mut self, name: &str, expr: &syn::Expr) -> bool {
        let Some(collection) = account_iterator_collection(expr) else {
            return false;
        };
        if !self.account_collections.contains(&collection) {
            return false;
        }
        self.account_iterators.insert(
            name.to_string(),
            AccountIteratorOrigin {
                next_index: INITIAL_ITERATOR_ACCOUNT_INDEX,
            },
        );
        true
    }

    fn next_account_info_origin(&mut self, expr: &syn::Expr) -> Option<AccountOrigin> {
        let iterator = next_account_info_iterator_name(expr)?;
        let origin = self.account_iterators.get_mut(&iterator)?;
        let account_index = origin.next_index;
        origin.next_index += 1;
        Some(AccountOrigin {
            account_index: Some(account_index),
        })
    }

    fn record_local_data_origin(&mut self, node: &syn::Local) {
        let Some(init) = node.init.as_ref() else {
            return;
        };
        let Some(local_name) = local_ident_name(node) else {
            return;
        };
        if let Some(read) = self.raw_data_read_from_expr(&init.expr) {
            self.data_origins.insert(local_name, read);
        }
    }

    fn record_destructured_account_origins(
        &mut self,
        pattern: &syn::Pat,
        collection_expr: &syn::Expr,
    ) -> bool {
        let Some(collection) = account_collection_name(collection_expr) else {
            return false;
        };
        if !self.account_collections.contains(&collection) {
            return false;
        }
        let mut recorded_any = false;
        for (index, name) in destructured_account_names(pattern).into_iter().enumerate() {
            let Some(name) = name else {
                continue;
            };
            self.account_values.insert(name.clone());
            self.account_origins.insert(
                name,
                AccountOrigin {
                    account_index: Some(index),
                },
            );
            recorded_any = true;
        }
        recorded_any
    }

    fn record_raw_data_read(
        &mut self,
        receiver: &syn::Expr,
        span: proc_macro2::Span,
    ) -> Option<NativeRawDataRead> {
        let read = self.raw_data_read(receiver, span)?;
        self.raw_data_reads.push(read.clone());
        Some(read)
    }

    fn raw_data_read(
        &self,
        receiver: &syn::Expr,
        span: proc_macro2::Span,
    ) -> Option<NativeRawDataRead> {
        let receiver_text = normalized_token_text(receiver);
        if !self.is_account_value_expression(receiver, &receiver_text) {
            return None;
        }
        Some(NativeRawDataRead {
            account_index: self.account_index_for_receiver(receiver, &receiver_text),
            receiver: Some(receiver_text),
            span,
        })
    }

    fn record_data_field_read(&mut self, receiver: &syn::Expr, span: proc_macro2::Span) {
        let Some(account_expr) = data_field_account_expr(receiver) else {
            return;
        };
        self.record_raw_data_read(account_expr, span);
    }

    fn raw_data_read_from_expr(&self, expr: &syn::Expr) -> Option<NativeRawDataRead> {
        match expr {
            syn::Expr::MethodCall(method_call)
                if RAW_ACCOUNT_DATA_METHODS.contains(&method_call.method.to_string().as_str()) =>
            {
                self.raw_data_read(method_call.receiver.as_ref(), method_call.method.span())
            }
            syn::Expr::Try(expr_try) => self.raw_data_read_from_expr(&expr_try.expr),
            syn::Expr::Unsafe(expr_unsafe) => tail_expr_from_block(&expr_unsafe.block)
                .and_then(|expr| self.raw_data_read_from_expr(expr)),
            syn::Expr::Block(expr_block) => tail_expr_from_block(&expr_block.block)
                .and_then(|expr| self.raw_data_read_from_expr(expr)),
            syn::Expr::Reference(reference) => self.raw_data_read_from_expr(&reference.expr),
            syn::Expr::Paren(paren) => self.raw_data_read_from_expr(&paren.expr),
            syn::Expr::Group(group) => self.raw_data_read_from_expr(&group.expr),
            _ => None,
        }
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

    fn record_owner_validation(&mut self, expr: Option<&syn::Expr>) {
        let Some(expr) = expr else {
            self.owner_validations.any = true;
            return;
        };
        let expression = normalized_token_text(expr);
        let account_index = self.account_index_for_receiver(expr, &expression);
        self.owner_validations.insert(expression, account_index);
    }

    fn record_discriminator_validation_for_data(&mut self, expr: &syn::Expr) {
        if let Some(read) = self.data_origin_from_expr(expr) {
            self.discriminator_validations.insert_read(&read);
        }
    }

    fn record_discriminator_validation_helper(&mut self, expr: Option<&syn::Expr>) {
        let Some(expr) = expr else {
            self.discriminator_validations.any = true;
            return;
        };
        if let Some(read) = self.data_origin_from_expr(expr) {
            self.discriminator_validations.insert_read(&read);
            return;
        }
        let expression = normalized_token_text(expr);
        let account_index = self.account_index_for_receiver(expr, &expression);
        self.discriminator_validations
            .insert(expression, account_index);
    }

    fn data_origin_from_expr(&self, expr: &syn::Expr) -> Option<NativeRawDataRead> {
        match expr {
            syn::Expr::Path(_) => path_ident_name(expr).and_then(|name| {
                self.data_origins.get(&name).cloned().or_else(|| {
                    self.account_origins
                        .get(&name)
                        .map(|origin| NativeRawDataRead {
                            span: expr.span(),
                            receiver: Some(name),
                            account_index: origin.account_index,
                        })
                })
            }),
            syn::Expr::Index(index) => self.data_origin_from_expr(&index.expr),
            syn::Expr::Reference(reference) => self.data_origin_from_expr(&reference.expr),
            syn::Expr::Paren(paren) => self.data_origin_from_expr(&paren.expr),
            syn::Expr::Group(group) => self.data_origin_from_expr(&group.expr),
            _ => None,
        }
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
        self.record_local_data_origin(node);
        visit::visit_local(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if RAW_ACCOUNT_DATA_METHODS.contains(&method.as_str()) {
            self.record_raw_data_read(node.receiver.as_ref(), node.method.span());
        }
        if RAW_ACCOUNT_DATA_FIELD_METHODS.contains(&method.as_str()) {
            self.record_data_field_read(node.receiver.as_ref(), node.method.span());
        }
        if RAW_DESERIALIZATION_CALLS.contains(&method.as_str()) {
            self.raw_deserializations.push(NativeRawDeserialization {
                span: node.method.span(),
                read: node
                    .args
                    .first()
                    .and_then(|expr| self.data_origin_from_expr(expr)),
            });
        }
        if method == "owner" || OWNER_VALIDATION_HELPERS.contains(&method.as_str()) {
            self.record_owner_validation(Some(node.receiver.as_ref()));
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Some(ident) = called_ident(&node.func) {
            if ident_matches_any(ident, RAW_DESERIALIZATION_CALLS) {
                self.raw_deserializations.push(NativeRawDeserialization {
                    span: ident.span(),
                    read: node
                        .args
                        .first()
                        .and_then(|expr| self.data_origin_from_expr(expr)),
                });
            }
            if ident_matches_any(ident, OWNER_VALIDATION_HELPERS) {
                self.record_owner_validation(node.args.first());
            }
            if ident_matches_any(ident, TYPE_VALIDATION_HELPERS) {
                self.record_discriminator_validation_helper(node.args.first());
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) {
            if expr_contains_discriminator(&node.left) {
                self.record_discriminator_validation_for_data(&node.right);
            }
            if expr_contains_discriminator(&node.right) {
                self.record_discriminator_validation_for_data(&node.left);
            }
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if member_is_named(&node.member, "owner") {
            self.record_owner_validation(Some(node.base.as_ref()));
        }
        visit::visit_expr_field(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        visit::visit_expr_path(self, node);
    }
}

#[derive(Default)]
struct AccountValidationSet {
    any: bool,
    expressions: HashSet<String>,
    indexes: HashSet<usize>,
}

impl AccountValidationSet {
    fn insert(&mut self, expression: String, account_index: Option<usize>) {
        self.expressions.insert(expression);
        if let Some(account_index) = account_index {
            self.indexes.insert(account_index);
        }
    }

    fn insert_read(&mut self, read: &NativeRawDataRead) {
        if let Some(receiver) = read.receiver.as_ref() {
            self.expressions.insert(receiver.clone());
        }
        if let Some(account_index) = read.account_index {
            self.indexes.insert(account_index);
        }
    }

    fn contains(&self, read: &NativeRawDataRead) -> bool {
        self.any
            || read
                .receiver
                .as_deref()
                .is_some_and(|receiver| self.expressions.contains(receiver))
            || read
                .account_index
                .is_some_and(|account_index| self.indexes.contains(&account_index))
    }
}

fn native_owner_validation_diagnostic(
    framework_kind: FrameworkKind,
    read: &NativeRawDataRead,
) -> Diagnostic {
    solana_code_quality_from_span(
        read.span,
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
            "programKind": framework_kind.program_kind_label(),
            "suggestion": "Check the account owner before interpreting raw account data.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    )
}

fn native_discriminator_validation_diagnostic(
    framework_kind: FrameworkKind,
    read: &NativeRawDataRead,
    span: proc_macro2::Span,
) -> Diagnostic {
    solana_code_quality_from_span(
        span,
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
            "programKind": framework_kind.program_kind_label(),
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
        syn::Type::Path(type_path) => type_path.path.segments.last().is_some_and(|segment| {
            matches!(
                segment.ident.to_string().as_str(),
                "AccountInfo" | "AccountView"
            )
        }),
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

fn destructured_account_names(pattern: &syn::Pat) -> Vec<Option<String>> {
    match pattern {
        syn::Pat::Slice(slice) => slice.elems.iter().map(pat_ident_name).collect(),
        syn::Pat::Reference(reference) => destructured_account_names(&reference.pat),
        syn::Pat::Type(pat_type) => destructured_account_names(&pat_type.pat),
        _ => Vec::new(),
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
        syn::Expr::MethodCall(method_call)
            if TRANSPARENT_ACCOUNT_ACCESS_METHODS
                .contains(&method_call.method.to_string().as_str()) =>
        {
            account_collection_access(&method_call.receiver)
        }
        syn::Expr::Try(expr_try) => account_collection_access(&expr_try.expr),
        syn::Expr::Group(group) => account_collection_access(&group.expr),
        syn::Expr::Paren(paren) => account_collection_access(&paren.expr),
        syn::Expr::Reference(reference) => account_collection_access(&reference.expr),
        _ => None,
    }
}

fn account_collection_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(_) => path_ident_name(expr),
        syn::Expr::MethodCall(method_call) if method_call.method == "as_slice" => {
            path_ident_name(&method_call.receiver)
        }
        syn::Expr::Group(group) => account_collection_name(&group.expr),
        syn::Expr::Paren(paren) => account_collection_name(&paren.expr),
        syn::Expr::Reference(reference) => account_collection_name(&reference.expr),
        _ => None,
    }
}

fn path_ident_name(expr: &syn::Expr) -> Option<String> {
    let syn::Expr::Path(path) = expr else {
        return None;
    };
    path.path.get_ident().map(ToString::to_string)
}

fn tail_expr_from_block(block: &syn::Block) -> Option<&syn::Expr> {
    match block.stmts.last()? {
        syn::Stmt::Expr(expr, None) => Some(expr),
        _ => None,
    }
}

fn expr_contains_discriminator(expr: &syn::Expr) -> bool {
    let mut visitor = DiscriminatorSearch { found: false };
    visitor.visit_expr(expr);
    visitor.found
}

struct DiscriminatorSearch {
    found: bool,
}

impl<'ast> Visit<'ast> for DiscriminatorSearch {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node
            .path
            .segments
            .iter()
            .any(|segment| segment.ident == "DISCRIMINATOR")
        {
            self.found = true;
        }
        visit::visit_expr_path(self, node);
    }
}
