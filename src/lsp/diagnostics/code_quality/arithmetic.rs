use {
    super::{local_ident, pat_ident, path_ident, ProgramKind},
    crate::{
        diagnostics::{
            diagnostic_from_span,
            lint::{
                run_lint_visitor_on_functions, Applicability, Confidence, FunctionBody,
                LintVisitor, Region,
            },
            registry::AnchorDiagnosticKind,
        },
        document::{ParsedDocument, SymbolRange},
        syntax::member_is_named,
    },
    std::collections::{HashMap, HashSet},
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
    },
    tower_lsp::lsp_types::Diagnostic,
};

const RULE: &str = "unchecked-arithmetic";
const TOPIC: &str = "seagrass/solana.code-quality.unchecked-arithmetic";
const TOKEN_ACCOUNT_GENERIC_TYPE: &str = "TokenAccount";
const TOKEN_AMOUNT_MEMBER: &str = "amount";
const ACCOUNTS_MEMBER: &str = "accounts";
const TO_ACCOUNT_INFO_METHOD: &str = "to_account_info";
const LAMPORT_ACCESS_METHODS: &[&str] = &[
    "lamports",
    "try_lamports",
    "try_borrow_lamports",
    "try_borrow_mut_lamports",
];

pub(super) fn diagnostics(document: &ParsedDocument, program_kind: ProgramKind) -> Vec<Diagnostic> {
    run_lint_visitor_on_functions(document, |function| {
        SemanticArithmeticVisitor::new(document, program_kind, function)
    })
}

struct SemanticArithmeticVisitor {
    program_kind: ProgramKind,
    context_token_account_fields: HashMap<String, HashSet<String>>,
    accounts_container_aliases: HashMap<String, HashSet<String>>,
    account_aliases: HashSet<String>,
    token_account_aliases: HashSet<String>,
    arithmetic_value_aliases: HashSet<String>,
    diagnostics: Vec<Diagnostic>,
}

struct AccountFieldEvidence {
    is_token_account: bool,
}

#[derive(Clone, Copy)]
enum ArithmeticValueEvidence {
    LamportsAccessor,
    TokenAccountAmount,
    DerivedLocal,
}

impl ArithmeticValueEvidence {
    const fn as_str(self) -> &'static str {
        match self {
            Self::LamportsAccessor => "lamports-accessor",
            Self::TokenAccountAmount => "token-account-amount",
            Self::DerivedLocal => "derived-local",
        }
    }
}

impl SemanticArithmeticVisitor {
    fn new(
        document: &ParsedDocument,
        program_kind: ProgramKind,
        function: FunctionBody<'_>,
    ) -> Self {
        let context_token_account_fields = context_token_account_fields(document, function.inputs);
        Self {
            program_kind,
            context_token_account_fields,
            accounts_container_aliases: HashMap::new(),
            account_aliases: HashSet::new(),
            token_account_aliases: HashSet::new(),
            arithmetic_value_aliases: HashSet::new(),
            diagnostics: Vec::new(),
        }
    }

    fn arithmetic_value_evidence(&self, expr: &syn::Expr) -> Option<ArithmeticValueEvidence> {
        match stripped_expression(expr) {
            syn::Expr::Path(path) => path.path.get_ident().and_then(|ident| {
                self.arithmetic_value_aliases
                    .contains(&ident.to_string())
                    .then_some(ArithmeticValueEvidence::DerivedLocal)
            }),
            syn::Expr::MethodCall(method)
                if is_lamport_access_method(&method.method)
                    && self.is_account_source(&method.receiver) =>
            {
                Some(ArithmeticValueEvidence::LamportsAccessor)
            }
            syn::Expr::Field(field)
                if member_is_named(&field.member, TOKEN_AMOUNT_MEMBER)
                    && self.is_token_account_source(&field.base) =>
            {
                Some(ArithmeticValueEvidence::TokenAccountAmount)
            }
            syn::Expr::Cast(cast) => self.arithmetic_value_evidence(&cast.expr),
            _ => None,
        }
    }

    fn is_account_source(&self, expr: &syn::Expr) -> bool {
        match stripped_expression(expr) {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .is_some_and(|ident| self.account_aliases.contains(&ident.to_string())),
            syn::Expr::Field(_) => context_account_field_evidence(
                expr,
                &self.context_token_account_fields,
                &self.accounts_container_aliases,
            )
            .is_some(),
            syn::Expr::MethodCall(method) if method.method == TO_ACCOUNT_INFO_METHOD => {
                self.is_account_source(&method.receiver)
            }
            _ => false,
        }
    }

    fn is_token_account_source(&self, expr: &syn::Expr) -> bool {
        match stripped_expression(expr) {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .is_some_and(|ident| self.token_account_aliases.contains(&ident.to_string())),
            syn::Expr::Field(_) => context_account_field_evidence(
                expr,
                &self.context_token_account_fields,
                &self.accounts_container_aliases,
            )
            .is_some_and(|evidence| evidence.is_token_account),
            _ => false,
        }
    }

    fn record_local_alias(&mut self, local: &syn::Local) {
        let Some(name) = local_ident(local).map(ToString::to_string) else {
            return;
        };
        let Some(init) = local.init.as_ref() else {
            self.clear_alias(&name);
            return;
        };
        self.record_alias_from_expr(&name, &init.expr);
    }

    fn record_assignment_alias(&mut self, assignment: &syn::ExprAssign) {
        let Some(name) = path_ident(&assignment.left).map(ToString::to_string) else {
            return;
        };
        self.record_alias_from_expr(&name, &assignment.right);
    }

    fn record_alias_from_expr(&mut self, name: &str, expr: &syn::Expr) {
        self.clear_alias(name);
        if let Some(token_account_fields) = self.accounts_container_token_fields(expr) {
            self.accounts_container_aliases
                .insert(name.to_string(), token_account_fields);
        }
        if self.is_account_source(expr) {
            self.account_aliases.insert(name.to_string());
        }
        if self.is_token_account_source(expr) {
            self.token_account_aliases.insert(name.to_string());
        }
        if self.arithmetic_value_evidence(expr).is_some() {
            self.arithmetic_value_aliases.insert(name.to_string());
        }
    }

    fn clear_alias(&mut self, name: &str) {
        self.accounts_container_aliases.remove(name);
        self.account_aliases.remove(name);
        self.token_account_aliases.remove(name);
        self.arithmetic_value_aliases.remove(name);
    }

    fn accounts_container_token_fields(&self, expr: &syn::Expr) -> Option<HashSet<String>> {
        accounts_container_token_fields(
            expr,
            &self.context_token_account_fields,
            &self.accounts_container_aliases,
        )
        .cloned()
    }
}

impl<'ast> LintVisitor<'ast> for SemanticArithmeticVisitor {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = TOPIC;

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for SemanticArithmeticVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_local(&mut self, node: &'ast syn::Local) {
        self.record_local_alias(node);
        visit::visit_local(self, node);
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        self.record_assignment_alias(node);
        visit::visit_expr_assign(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if is_unchecked_semantic_arithmetic_operator(&node.op) {
            let evidence = self
                .arithmetic_value_evidence(&node.left)
                .or_else(|| self.arithmetic_value_evidence(&node.right));
            if let Some(evidence) = evidence {
                self.diagnostics.push(unchecked_arithmetic_diagnostic(
                    self.program_kind,
                    node.op.span(),
                    evidence,
                ));
            }
        }
        visit::visit_expr_binary(self, node);
    }
}

fn unchecked_arithmetic_diagnostic(
    program_kind: ProgramKind,
    span: proc_macro2::Span,
    evidence: ArithmeticValueEvidence,
) -> Diagnostic {
    diagnostic_from_span(
        span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Use checked arithmetic for lamports or token amount math.".to_string(),
        Some(serde_json::json!({
            "rule": RULE,
            "attack": RULE,
            "topic": TOPIC,
            "programKind": program_kind.as_str(),
            "evidenceSource": evidence.as_str(),
            "quickfix": "checked-arithmetic",
            "suggestion": "Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div`, or `checked_rem` and return an explicit program error on overflow, underflow, or invalid arithmetic.",
            "absorbedFrom": "semantic-local-analysis",
        })),
    )
}

fn is_unchecked_semantic_arithmetic_operator(op: &syn::BinOp) -> bool {
    matches!(
        op,
        syn::BinOp::Add(_)
            | syn::BinOp::Sub(_)
            | syn::BinOp::Mul(_)
            | syn::BinOp::Div(_)
            | syn::BinOp::Rem(_)
            | syn::BinOp::AddAssign(_)
            | syn::BinOp::SubAssign(_)
            | syn::BinOp::MulAssign(_)
            | syn::BinOp::DivAssign(_)
            | syn::BinOp::RemAssign(_)
    )
}

fn is_lamport_access_method(method: &syn::Ident) -> bool {
    LAMPORT_ACCESS_METHODS
        .iter()
        .any(|candidate| method == *candidate)
}

fn context_token_account_fields(
    document: &ParsedDocument,
    inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
) -> HashMap<String, HashSet<String>> {
    inputs
        .iter()
        .filter_map(|arg| {
            let syn::FnArg::Typed(pat_type) = arg else {
                return None;
            };
            let binding = pat_ident(&pat_type.pat)?.to_string();
            let token_fields =
                token_account_fields(document, &context_type_argument(&pat_type.ty)?);
            Some((binding, token_fields))
        })
        .collect()
}

fn context_type_argument(ty: &syn::Type) -> Option<String> {
    let ty = match ty {
        syn::Type::Reference(reference) => reference.elem.as_ref(),
        _ => ty,
    };
    let syn::Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.iter().find(|segment| {
        segment.ident == "Context" || segment.ident.to_string().ends_with("Context")
    })?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        syn::GenericArgument::Type(syn::Type::Path(path)) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        _ => None,
    })
}

fn token_account_fields(document: &ParsedDocument, context_struct: &str) -> HashSet<String> {
    document
        .symbols()
        .accounts_structs
        .get(context_struct)
        .into_iter()
        .flat_map(|accounts| accounts.fields.iter())
        .filter(|field| is_token_account_field(field))
        .map(|field| field.name.clone())
        .collect()
}

fn is_token_account_field(field: &SymbolRange) -> bool {
    matches!(
        field.type_name.as_deref(),
        Some("Account" | "InterfaceAccount")
    ) && field
        .generic_type_names
        .iter()
        .any(|generic| generic == TOKEN_ACCOUNT_GENERIC_TYPE)
}

fn context_account_field_evidence(
    expr: &syn::Expr,
    context_token_account_fields: &HashMap<String, HashSet<String>>,
    accounts_container_aliases: &HashMap<String, HashSet<String>>,
) -> Option<AccountFieldEvidence> {
    let syn::Expr::Field(field) = stripped_expression(expr) else {
        return None;
    };
    let field_name = member_name(&field.member)?;
    let token_account_fields = accounts_container_token_fields(
        &field.base,
        context_token_account_fields,
        accounts_container_aliases,
    )?;
    Some(AccountFieldEvidence {
        is_token_account: token_account_fields.contains(&field_name),
    })
}

fn accounts_container_token_fields<'a>(
    expr: &syn::Expr,
    context_token_account_fields: &'a HashMap<String, HashSet<String>>,
    accounts_container_aliases: &'a HashMap<String, HashSet<String>>,
) -> Option<&'a HashSet<String>> {
    match stripped_expression(expr) {
        syn::Expr::Path(path) => path
            .path
            .get_ident()
            .and_then(|ident| accounts_container_aliases.get(&ident.to_string())),
        syn::Expr::Field(field) if member_is_named(&field.member, ACCOUNTS_MEMBER) => {
            path_ident(&field.base)
                .and_then(|ident| context_token_account_fields.get(&ident.to_string()))
        }
        _ => None,
    }
}

fn member_name(member: &syn::Member) -> Option<String> {
    match member {
        syn::Member::Named(ident) => Some(ident.to_string()),
        syn::Member::Unnamed(_) => None,
    }
}

fn stripped_expression(expr: &syn::Expr) -> &syn::Expr {
    let mut current = expr;
    loop {
        current = match current {
            syn::Expr::Paren(paren) => &paren.expr,
            syn::Expr::Group(group) => &group.expr,
            syn::Expr::Reference(reference) => &reference.expr,
            syn::Expr::Try(expr_try) => &expr_try.expr,
            syn::Expr::Unary(unary) => &unary.expr,
            _ => return current,
        };
    }
}
