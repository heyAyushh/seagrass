#![allow(deprecated)]

use {
    crate::{range::range_from_span, syntax::RustSyntax},
    proc_macro2::Span,
    quote::ToTokens,
    std::collections::{HashMap, HashSet},
    syn::{
        spanned::Spanned, Attribute, FnArg, GenericArgument, Item, ItemFn, ItemStruct, PatType,
        Path, PathArguments, Token, Type, UseTree, Visibility,
    },
    tower_lsp::lsp_types::{Position, Range},
};

mod account_attribute;
mod account_usage;
mod associated_values;
mod symbols;

pub use {
    account_attribute::{AccountAttributeCursor, AccountAttributeSlot},
    associated_values::{AssociatedValueKind, AssociatedValueRange},
    symbols::document_symbols,
};

#[derive(Debug)]
pub struct ParsedDocument {
    source: String,
    tree_sitter: Option<RustSyntax>,
    syntax: syn::File,
    symbols: AnchorSymbols,
}

impl ParsedDocument {
    pub fn parse(source: impl Into<String>) -> Result<Self, syn::Error> {
        let source = source.into();
        let tree_sitter = RustSyntax::parse(&source);
        let syntax = syn::parse_file(&source)?;
        let symbols = AnchorSymbols::from_items(&syntax.items);

        Ok(Self {
            source,
            tree_sitter,
            syntax,
            symbols,
        })
    }

    pub fn parse_or_empty(source: impl Into<String>) -> Self {
        let source = source.into();
        Self::parse(source.clone()).unwrap_or_else(|_| Self {
            tree_sitter: RustSyntax::parse(&source),
            source,
            syntax: syn::File {
                shebang: None,
                attrs: Vec::new(),
                items: Vec::new(),
            },
            symbols: AnchorSymbols::default(),
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn syntax(&self) -> &syn::File {
        &self.syntax
    }

    pub fn tree_sitter(&self) -> Option<&RustSyntax> {
        self.tree_sitter.as_ref()
    }

    pub fn symbols(&self) -> &AnchorSymbols {
        &self.symbols
    }

    pub fn account_attribute_cursor(&self, position: Position) -> Option<AccountAttributeCursor> {
        let cursor = self
            .tree_sitter
            .as_ref()
            .and_then(|syntax| syntax.account_attribute_cursor_at_position(&self.source, position))
            .or_else(|| AccountAttributeCursor::from_source(&self.source, position));

        cursor.map(|mut cursor| {
            cursor.field_name =
                account_attribute::account_attribute_field_name(self, position.line);
            cursor
        })
    }

    pub fn is_in_account_attribute(&self, position: Position) -> bool {
        self.account_attribute_cursor(position).is_some()
    }
}

#[derive(Debug, Default)]
pub struct AnchorSymbols {
    pub all_structs: HashMap<String, SymbolRange>,
    pub accounts_structs: HashMap<String, SymbolRange>,
    pub account_data_structs: HashMap<String, SymbolRange>,
    pub constants: Vec<NamedRange>,
    pub imported_names: Vec<NamedRange>,
    pub value_items: Vec<NamedRange>,
    pub associated_value_items: HashMap<String, Vec<AssociatedValueRange>>,
    pub derived_init_space_types: HashSet<String>,
    pub instructions: Vec<InstructionSymbol>,
    pub functions: Vec<InstructionSymbol>,
    pub context_references: Vec<ContextReference>,
    pub declared_program_id: Option<DeclaredProgramId>,
}

impl AnchorSymbols {
    fn from_items(items: &[Item]) -> Self {
        let mut symbols = Self::default();

        for item in items {
            match item {
                Item::Struct(item_struct) => {
                    let symbol = SymbolRange::from_struct(item_struct);
                    let name = item_struct.ident.to_string();
                    symbols.all_structs.insert(name.clone(), symbol.clone());

                    if derives_accounts(&item_struct.attrs) {
                        symbols
                            .accounts_structs
                            .insert(name.clone(), symbol.clone());
                    }
                    if has_attr(&item_struct.attrs, "account") {
                        symbols.account_data_structs.insert(name, symbol);
                    }
                    if associated_values::derives_init_space(&item_struct.attrs) {
                        symbols
                            .derived_init_space_types
                            .insert(item_struct.ident.to_string());
                    }
                }
                Item::Mod(item_mod) if has_attr(&item_mod.attrs, "program") => {
                    if let Some((_, mod_items)) = &item_mod.content {
                        for item in mod_items {
                            if let Item::Fn(item_fn) = item {
                                let function = instruction_symbol(item_fn);
                                let context_reference = function.context.clone();
                                symbols.instructions.push(function);
                                if let Some(reference) = context_reference {
                                    symbols.context_references.push(reference);
                                }
                            }
                        }
                    }
                }
                Item::Fn(item_fn) if is_anchor_helper_function(item_fn) => {
                    symbols.value_items.push(item_fn_range(item_fn));
                    let function = instruction_symbol(item_fn);
                    if let Some(reference) = function.context.clone() {
                        symbols.context_references.push(reference);
                    }
                    symbols.functions.push(function);
                }
                Item::Fn(item_fn) => {
                    symbols.value_items.push(item_fn_range(item_fn));
                }
                Item::Const(item_const) => {
                    let item = NamedRange {
                        name: item_const.ident.to_string(),
                        range: range_from_span(item_const.ident.span()),
                    };
                    symbols.constants.push(item.clone());
                    symbols.value_items.push(item);
                }
                Item::Static(item_static) => {
                    symbols.value_items.push(NamedRange {
                        name: item_static.ident.to_string(),
                        range: range_from_span(item_static.ident.span()),
                    });
                }
                Item::Use(item_use) => {
                    collect_imported_names(&item_use.tree, &mut symbols.imported_names);
                }
                Item::Impl(item_impl) => {
                    associated_values::collect_from_impl(
                        item_impl,
                        &mut symbols.associated_value_items,
                    );
                }
                Item::Macro(item_macro)
                    if path_last_is_ident(&item_macro.mac.path, "declare_id") =>
                {
                    if let Some(declared) = declared_program_id(item_macro) {
                        symbols.declared_program_id = Some(declared);
                    }
                }
                _ => {}
            }
        }

        symbols
    }

    pub fn knows_type(&self, name: &str) -> bool {
        self.all_structs.contains_key(name)
            || self
                .context_references
                .iter()
                .any(|reference| reference.name == name)
    }

    pub fn callable_functions(&self) -> impl Iterator<Item = &InstructionSymbol> {
        self.instructions.iter().chain(self.functions.iter())
    }

    pub fn type_has_associated_value(&self, type_name: &str, value_name: &str) -> bool {
        self.associated_value_items
            .get(type_name)
            .is_some_and(|items| items.iter().any(|item| item.name == value_name))
            || (associated_values::is_generated_init_space_value(value_name)
                && self.derived_init_space_types.contains(type_name))
    }
}

fn item_fn_range(item_fn: &ItemFn) -> NamedRange {
    NamedRange {
        name: item_fn.sig.ident.to_string(),
        range: range_from_span(item_fn.sig.ident.span()),
    }
}

fn collect_imported_names(tree: &UseTree, names: &mut Vec<NamedRange>) {
    match tree {
        UseTree::Name(name) => names.push(NamedRange {
            name: name.ident.to_string(),
            range: range_from_span(name.ident.span()),
        }),
        UseTree::Rename(rename) => names.push(NamedRange {
            name: rename.rename.to_string(),
            range: range_from_span(rename.rename.span()),
        }),
        UseTree::Path(path) => {
            names.push(NamedRange {
                name: path.ident.to_string(),
                range: range_from_span(path.ident.span()),
            });
            collect_imported_names(&path.tree, names);
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                collect_imported_names(tree, names);
            }
        }
        UseTree::Glob(_) => {}
    }
}

#[derive(Debug, Clone)]
pub struct SymbolRange {
    pub name: String,
    pub range: Range,
    pub selection_range: Range,
    pub fields: Vec<SymbolRange>,
    pub type_name: Option<String>,
    pub type_range: Option<Range>,
    pub generic_type_names: Vec<String>,
    pub generic_type_ranges: Vec<NamedRange>,
    pub is_optional: bool,
    pub account_constraints: Vec<AccountConstraint>,
    pub pda_constraint: Option<PdaConstraint>,
    pub instruction_arguments: Vec<InstructionAttributeArgument>,
    pub derive_accounts_range: Option<Range>,
}

impl SymbolRange {
    fn from_struct(item_struct: &ItemStruct) -> Self {
        Self {
            name: item_struct.ident.to_string(),
            range: range_from_span(item_struct.span()),
            selection_range: range_from_span(item_struct.ident.span()),
            fields: field_symbols(item_struct),
            type_name: None,
            type_range: None,
            generic_type_names: Vec::new(),
            generic_type_ranges: Vec::new(),
            is_optional: false,
            account_constraints: Vec::new(),
            pda_constraint: None,
            instruction_arguments: instruction_attribute_arguments(&item_struct.attrs),
            derive_accounts_range: derive_accounts_range(&item_struct.attrs),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedRange {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct AccountConstraint {
    pub text: String,
    pub range: Range,
    pub pda: Option<PdaConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdaConstraint {
    pub is_init: bool,
    pub seeds: PdaSeeds,
    pub bump: PdaBump,
    pub program_seed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdaSeeds {
    List(Vec<String>),
    Expr(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdaBump {
    Canonical,
    Explicit(String),
    Missing,
}

#[derive(Debug, Clone)]
pub struct InstructionAttributeArgument {
    pub name: String,
    pub range: Range,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InstructionSymbol {
    pub name: String,
    pub range: Range,
    pub selection_range: Range,
    pub context: Option<ContextReference>,
    pub arguments: Vec<InstructionArgument>,
    pub function_calls: Vec<FunctionCall>,
    pub account_usages: Vec<AccountUsage>,
    pub account_data_field_usages: Vec<AccountDataFieldUsage>,
    pub account_path_usages: Vec<AccountPathUsage>,
    pub cpi_program_usages: Vec<AccountUsage>,
    pub signer_usages: Vec<AccountUsage>,
    pub signer_checks: Vec<AccountUsage>,
    pub account_key_comparisons: Vec<AccountKeyComparison>,
    pub token_account_unpack_usages: Vec<AccountUsage>,
}

#[derive(Debug, Clone)]
pub struct ContextReference {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct InstructionArgument {
    pub name: String,
    pub range: Range,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionCall {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountUsage {
    pub name: String,
    pub range: Range,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountDataFieldUsage {
    pub account: String,
    pub field: String,
    pub range: Range,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountPathUsage {
    pub segments: Vec<NamedRange>,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountKeyComparison {
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredProgramId {
    pub value: String,
    pub range: Range,
}

pub fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

fn path_last_is_ident(path: &Path, name: &str) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

pub fn derives_accounts(attrs: &[Attribute]) -> bool {
    derive_accounts_range(attrs).is_some()
}

fn derive_accounts_range(attrs: &[Attribute]) -> Option<Range> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .find_map(|attr| {
            let paths = attr
                .parse_args_with(
                    syn::punctuated::Punctuated::<Path, syn::Token![,]>::parse_terminated,
                )
                .ok()?;
            paths
                .iter()
                .find(|path| path.is_ident("Accounts"))
                .map(|path| range_from_span(path.span()))
        })
}

fn instruction_symbol(item_fn: &ItemFn) -> InstructionSymbol {
    let context_reference = context_accounts_arg(item_fn);
    let account_evidence = account_usage::account_evidence(item_fn);
    InstructionSymbol {
        name: item_fn.sig.ident.to_string(),
        range: range_from_span(item_fn.span()),
        selection_range: range_from_span(item_fn.sig.ident.span()),
        context: context_reference,
        arguments: instruction_arguments(item_fn),
        function_calls: account_evidence.function_calls,
        account_usages: account_evidence.usages,
        account_data_field_usages: account_evidence.data_field_usages,
        account_path_usages: account_evidence.account_path_usages,
        cpi_program_usages: account_evidence.cpi_program_usages,
        signer_usages: account_evidence.signer_usages,
        signer_checks: account_evidence.signer_checks,
        account_key_comparisons: account_evidence.account_key_comparisons,
        token_account_unpack_usages: account_evidence.token_account_unpack_usages,
    }
}

fn is_anchor_helper_function(item_fn: &ItemFn) -> bool {
    matches!(item_fn.vis, Visibility::Public(_)) || context_accounts_arg(item_fn).is_some()
}

fn field_symbols(item_struct: &ItemStruct) -> Vec<SymbolRange> {
    let syn::Fields::Named(fields) = &item_struct.fields else {
        return Vec::new();
    };
    let parser_pdas = parser_pda_constraints(item_struct);

    fields
        .named
        .iter()
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            let (field_ty, is_optional) = account_field_type(&field.ty);
            let generic_type_ranges = generic_type_ranges(field_ty);
            let account_constraints = account_constraints(&field.attrs);
            let pda_constraint = parser_pdas.get(&ident.to_string()).cloned().or_else(|| {
                account_constraints
                    .iter()
                    .find_map(|constraint| constraint.pda.clone())
            });
            Some(SymbolRange {
                name: ident.to_string(),
                range: range_from_span(field.span()),
                selection_range: range_from_span(ident.span()),
                fields: Vec::new(),
                type_name: type_name(field_ty),
                type_range: type_range(field_ty),
                generic_type_names: generic_type_ranges
                    .iter()
                    .map(|range| range.name.clone())
                    .collect(),
                generic_type_ranges,
                is_optional,
                account_constraints,
                pda_constraint,
                instruction_arguments: Vec::new(),
                derive_accounts_range: None,
            })
        })
        .collect()
}

fn parser_pda_constraints(item_struct: &ItemStruct) -> HashMap<String, PdaConstraint> {
    let Ok(accounts) = anchor_syn::parser::accounts::parse(item_struct) else {
        return HashMap::new();
    };

    accounts
        .fields
        .iter()
        .filter_map(|field| {
            let (name, constraints) = match field {
                anchor_syn::AccountField::Field(field) => {
                    (field.ident.to_string(), &field.constraints)
                }
                anchor_syn::AccountField::CompositeField(field) => {
                    (field.ident.to_string(), &field.constraints)
                }
            };
            constraints
                .seeds
                .as_ref()
                .map(|seeds| (name, pda_constraint_from_group(seeds)))
        })
        .collect()
}

fn pda_constraint_from_group(seeds: &anchor_syn::ConstraintSeedsGroup) -> PdaConstraint {
    PdaConstraint {
        is_init: seeds.is_init,
        seeds: pda_seeds_from_expr(&seeds.seeds),
        bump: seeds.bump.as_ref().map_or(PdaBump::Canonical, |expr| {
            PdaBump::Explicit(normalize_token_text(&expr.to_token_stream().to_string()))
        }),
        program_seed: seeds
            .program_seed
            .as_ref()
            .map(|expr| normalize_token_text(&expr.to_token_stream().to_string())),
    }
}

fn pda_seeds_from_expr(seeds: &anchor_syn::SeedsExpr) -> PdaSeeds {
    match seeds {
        anchor_syn::SeedsExpr::List(list) => PdaSeeds::List(
            list.iter()
                .map(|expr| normalize_token_text(&expr.to_token_stream().to_string()))
                .collect(),
        ),
        anchor_syn::SeedsExpr::Expr(expr) => {
            PdaSeeds::Expr(normalize_token_text(&expr.to_token_stream().to_string()))
        }
    }
}

fn account_field_type(ty: &Type) -> (&Type, bool) {
    let Type::Path(type_path) = ty else {
        return (ty, false);
    };
    let Some(segment) = type_path.path.segments.last() else {
        return (ty, false);
    };
    if segment.ident != "Option" {
        return (ty, false);
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return (ty, false);
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return (ty, false);
    };
    (inner, true)
}

fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn type_range(ty: &Type) -> Option<Range> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| range_from_span(segment.ident.span()))
}

fn generic_type_ranges(ty: &Type) -> Vec<NamedRange> {
    let Type::Path(type_path) = ty else {
        return Vec::new();
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Vec::new();
    };
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Vec::new();
    };

    args.args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(Type::Path(type_path)) => {
                type_path.path.segments.last().map(|segment| NamedRange {
                    name: segment.ident.to_string(),
                    range: range_from_span(segment.ident.span()),
                })
            }
            _ => None,
        })
        .collect()
}

fn account_constraints(attrs: &[Attribute]) -> Vec<AccountConstraint> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("account"))
        .map(|attr| AccountConstraint {
            text: normalize_token_text(&attr.meta.to_token_stream().to_string()),
            range: range_from_span(attr.span()),
            pda: pda_constraint(attr),
        })
        .collect()
}

fn pda_constraint(attr: &Attribute) -> Option<PdaConstraint> {
    let tokens = attr
        .parse_args_with(
            syn::punctuated::Punctuated::<anchor_syn::ConstraintToken, Token![,]>::parse_terminated,
        )
        .ok()?;

    let mut seeds = None;
    let mut bump = PdaBump::Missing;
    let mut program_seed = None;

    for token in tokens {
        match token {
            anchor_syn::ConstraintToken::Seeds(seed_context) => {
                seeds = Some(match &seed_context.seeds {
                    anchor_syn::SeedsExpr::List(list) => PdaSeeds::List(
                        list.iter()
                            .map(|expr| normalize_token_text(&expr.to_token_stream().to_string()))
                            .collect(),
                    ),
                    anchor_syn::SeedsExpr::Expr(expr) => {
                        PdaSeeds::Expr(normalize_token_text(&expr.to_token_stream().to_string()))
                    }
                });
            }
            anchor_syn::ConstraintToken::Bump(bump_context) => {
                bump = bump_context
                    .bump
                    .as_ref()
                    .map_or(PdaBump::Canonical, |expr| {
                        PdaBump::Explicit(normalize_token_text(&expr.to_token_stream().to_string()))
                    });
            }
            anchor_syn::ConstraintToken::ProgramSeed(program_context) => {
                program_seed = Some(normalize_token_text(
                    &program_context.program_seed.to_token_stream().to_string(),
                ));
            }
            _ => {}
        }
    }

    seeds.map(|seeds| PdaConstraint {
        is_init: false,
        seeds,
        bump,
        program_seed,
    })
}

fn instruction_attribute_arguments(attrs: &[Attribute]) -> Vec<InstructionAttributeArgument> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("instruction"))
        .flat_map(|attr| {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<PatType, syn::Token![,]>::parse_terminated,
            )
            .unwrap_or_default()
            .into_iter()
            .filter_map(|pat_type| {
                let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
                    return None;
                };
                Some(InstructionAttributeArgument {
                    name: pat_ident.ident.to_string(),
                    range: range_from_span(pat_ident.ident.span()),
                    type_name: type_name(pat_type.ty.as_ref()),
                })
            })
        })
        .collect()
}

fn normalize_token_text(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn context_accounts_arg(item_fn: &ItemFn) -> Option<ContextReference> {
    item_fn.sig.inputs.iter().find_map(|arg| match arg {
        FnArg::Typed(pat_type) => context_accounts_type(pat_type),
        FnArg::Receiver(_) => None,
    })
}

fn instruction_arguments(item_fn: &ItemFn) -> Vec<InstructionArgument> {
    item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            let FnArg::Typed(pat_type) = arg else {
                return None;
            };
            if context_accounts_type(pat_type).is_some() {
                return None;
            }
            let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
                return None;
            };
            Some(InstructionArgument {
                name: pat_ident.ident.to_string(),
                range: range_from_span(pat_ident.ident.span()),
                type_name: type_name(pat_type.ty.as_ref()),
            })
        })
        .collect()
}

fn declared_program_id(item_macro: &syn::ItemMacro) -> Option<DeclaredProgramId> {
    let literal = syn::parse2::<syn::LitStr>(item_macro.mac.tokens.clone()).ok()?;
    Some(DeclaredProgramId {
        value: literal.value(),
        range: string_literal_value_range(literal.span()),
    })
}

fn string_literal_value_range(span: Span) -> Range {
    let mut range = range_from_span(span);
    if range.start.line == range.end.line
        && range.end.character > range.start.character.saturating_add(1)
    {
        range.start.character += 1;
        range.end.character = range.end.character.saturating_sub(1);
    }
    range
}

fn context_accounts_type(pat_type: &PatType) -> Option<ContextReference> {
    context_accounts_type_inner(pat_type.ty.as_ref())
}

fn context_accounts_type_inner(ty: &Type) -> Option<ContextReference> {
    let ty = match ty {
        Type::Reference(reference) => reference.elem.as_ref(),
        _ => ty,
    };
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.iter().find(|segment| {
        segment.ident == "Context" || segment.ident.to_string().ends_with("Context")
    })?;
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };

    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(Type::Path(account_path)) => {
            account_path
                .path
                .segments
                .last()
                .map(|segment| ContextReference {
                    name: segment.ident.to_string(),
                    range: range_from_span(segment.ident.span()),
                })
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests;
