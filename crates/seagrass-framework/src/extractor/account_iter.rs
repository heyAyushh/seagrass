use {
    crate::{
        native_rules::common::{
            account_iterator_collection, compact_token_text, local_ident_name,
            next_account_info_iterator_name, pat_ident_name, AccountIteratorOrigin,
            INITIAL_ITERATOR_ACCOUNT_INDEX,
        },
        range::range_from_span,
        semantic::{
            AccountField, AccountType, AccountsStruct, Check, CheckKind, Constraint,
            ConstraintValue, ExtractionConfidence, Instruction, Populated, PopulatedFields,
            SemanticModel,
        },
    },
    std::collections::{HashMap, HashSet},
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
    },
};

#[derive(Default)]
pub(crate) struct NativeAccountExtractor {
    accounts_structs: Vec<Populated<AccountsStruct>>,
    instructions: Vec<Populated<Instruction>>,
}

impl NativeAccountExtractor {
    pub(crate) fn finish(self) -> SemanticModel {
        SemanticModel {
            instructions: self.instructions,
            accounts_structs: self.accounts_structs,
            ..SemanticModel::default()
        }
    }
}

impl<'ast> Visit<'ast> for NativeAccountExtractor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let mut function = FunctionAccountExtractor::new(node);
        function.visit_block(&node.block);
        if let Some((accounts_struct, instruction)) = function.finish() {
            self.accounts_structs.push(accounts_struct);
            self.instructions.push(instruction);
        }
    }
}

struct FunctionAccountExtractor<'a> {
    function: &'a syn::ItemFn,
    account_collections: HashSet<String>,
    account_iterators: HashMap<String, AccountIteratorOrigin>,
    fields: Vec<AccountField>,
    signer_checks: Vec<Check>,
    signer_requirements: HashSet<String>,
}

impl<'a> FunctionAccountExtractor<'a> {
    fn new(function: &'a syn::ItemFn) -> Self {
        Self {
            function,
            account_collections: function
                .sig
                .inputs
                .iter()
                .filter_map(account_collection_arg)
                .collect(),
            account_iterators: HashMap::new(),
            fields: Vec::new(),
            signer_checks: Vec::new(),
            signer_requirements: HashSet::new(),
        }
    }

    fn finish(mut self) -> Option<(Populated<AccountsStruct>, Populated<Instruction>)> {
        if self.fields.is_empty() {
            return None;
        }
        self.apply_signer_requirements();
        let accounts_name = self.function.sig.ident.to_string();
        let mut instruction_fields = PopulatedFields::default();
        if !self.signer_checks.is_empty() {
            instruction_fields.set(PopulatedFields::SIGNER_CHECKS);
        }
        Some((
            Populated::new(
                AccountsStruct {
                    name: accounts_name.clone(),
                    fields: self.fields,
                    composite_refs: Vec::new(),
                },
                ExtractionConfidence::IdiomBased,
            ),
            Populated::with_fields(
                Instruction {
                    name: self.function.sig.ident.to_string(),
                    context_type: Some(accounts_name),
                    parameters: Vec::new(),
                    cpi_calls: Vec::new(),
                    signer_checks: self.signer_checks,
                    owner_checks: Vec::new(),
                    discriminator_checks: Vec::new(),
                },
                ExtractionConfidence::IdiomBased,
                instruction_fields,
            ),
        ))
    }

    fn record_local_account(&mut self, node: &syn::Local) {
        let Some(init) = node.init.as_ref() else {
            return;
        };
        let Some(local_name) = local_ident_name(node) else {
            return;
        };
        if self.record_account_iterator_origin(&local_name, &init.expr) {
            return;
        }
        if self.next_account_info_iterator_name(&init.expr).is_some() {
            self.fields.push(AccountField {
                name: local_name,
                source_range: range_from_span(node.pat.span()),
                account_type: AccountType::RawAccountInfo,
                constraints: Vec::new(),
                token_interface_candidate: false,
            });
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

    fn next_account_info_iterator_name(&mut self, expr: &syn::Expr) -> Option<String> {
        let iterator = next_account_info_iterator_name(expr)?;
        let origin = self.account_iterators.get_mut(&iterator)?;
        origin.next_index += 1;
        Some(iterator)
    }

    fn record_signer_check(&mut self, subject: &syn::Expr, span: proc_macro2::Span) {
        let subject_ref = compact_token_text(subject);
        if self.fields.iter().any(|field| field.name == subject_ref) {
            self.signer_checks.push(Check {
                kind: CheckKind::Signer,
                subject_ref: Some(subject_ref),
                source_range: range_from_span(span),
            });
        }
    }

    fn record_signer_requirement(&mut self, subject: Option<&syn::Expr>) {
        let Some(subject) = subject.and_then(signer_subject_name) else {
            return;
        };
        self.signer_requirements.insert(subject);
    }

    fn apply_signer_requirements(&mut self) {
        for field in &mut self.fields {
            if self.signer_requirements.contains(&field.name) {
                field.constraints.push(Constraint {
                    key: "requires_signer".to_string(),
                    value: ConstraintValue::Absent,
                    source_range: field.source_range,
                });
            }
        }
    }
}

impl<'ast> Visit<'ast> for FunctionAccountExtractor<'_> {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        self.record_local_account(node);
        visit::visit_local(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if matches!(&node.member, syn::Member::Named(ident) if ident == "is_signer") {
            self.record_signer_check(&node.base, node.member.span());
        }
        visit::visit_expr_field(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "is_signer" {
            self.record_signer_check(&node.receiver, node.method.span());
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if account_meta_signer_constructor(node) {
            self.record_signer_requirement(node.args.first());
        }
        visit::visit_expr_call(self, node);
    }
}

fn account_collection_arg(arg: &syn::FnArg) -> Option<String> {
    let syn::FnArg::Typed(pat_type) = arg else {
        return None;
    };
    let name = pat_ident_name(&pat_type.pat)?;
    type_mentions_account_info(&pat_type.ty).then_some(name)
}

fn type_mentions_account_info(ty: &syn::Type) -> bool {
    compact_token_text(ty).contains("AccountInfo")
}

fn account_meta_signer_constructor(node: &syn::ExprCall) -> bool {
    let syn::Expr::Path(path) = node.func.as_ref() else {
        return false;
    };
    let segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    (path_ends_with(&segments, &["AccountMeta", "new"])
        || path_ends_with(&segments, &["AccountMeta", "new_readonly"]))
        && node.args.iter().nth(1).is_some_and(bool_literal_is_true)
        || path_ends_with(&segments, &["InstructionAccount", "readonly_signer"])
        || path_ends_with(&segments, &["InstructionAccount", "writable_signer"])
}

fn path_ends_with(path: &[String], suffix: &[&str]) -> bool {
    path.len() >= suffix.len()
        && path[path.len() - suffix.len()..]
            .iter()
            .map(String::as_str)
            .eq(suffix.iter().copied())
}

fn bool_literal_is_true(expr: &syn::Expr) -> bool {
    matches!(
        expr,
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Bool(value),
            ..
        }) if value.value
    )
}

fn signer_subject_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(_) => Some(compact_token_text(expr)),
        syn::Expr::Field(field) if matches!(&field.member, syn::Member::Named(ident) if ident == "key") => {
            signer_subject_name(&field.base)
        }
        syn::Expr::MethodCall(method) if method.method == "key" || method.method == "address" => {
            signer_subject_name(&method.receiver)
        }
        syn::Expr::Unary(unary) => signer_subject_name(&unary.expr),
        syn::Expr::Reference(reference) => signer_subject_name(&reference.expr),
        syn::Expr::Paren(paren) => signer_subject_name(&paren.expr),
        syn::Expr::Group(group) => signer_subject_name(&group.expr),
        _ => None,
    }
}
