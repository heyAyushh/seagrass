#![allow(deprecated)]

use {
    crate::{anchor::idioms, range::range_from_span, syntax::RustSyntax},
    proc_macro2::Span,
    quote::ToTokens,
    std::collections::{HashMap, HashSet},
    syn::{
        spanned::Spanned, Attribute, Fields, FnArg, GenericArgument, Item, ItemEnum, ItemFn,
        ItemStruct, PatType, Path, PathArguments, Token, Type, Visibility,
    },
    tower_lsp::lsp_types::{Position, Range},
};

mod account_attribute;
mod account_field_type;
mod account_usage;
mod associated_values;
mod error_codes;
mod field_types;
mod imports;
mod symbols;

use {
    account_field_type::account_field_type,
    field_types::{generic_type_ranges, type_name, type_range},
    imports::{collect_imported_names, has_glob_in_use_tree, has_local_glob_in_use_tree},
};

pub use {
    account_attribute::{AccountAttributeCursor, AccountAttributeSlot},
    associated_values::{is_generated_init_space_value, AssociatedValueKind, AssociatedValueRange},
    error_codes::ErrorCodeEnum,
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
    pub enums: HashMap<String, SymbolRange>,
    pub accounts_structs: HashMap<String, SymbolRange>,
    pub account_data_structs: HashMap<String, SymbolRange>,
    pub constants: Vec<NamedRange>,
    pub imported_names: Vec<NamedRange>,
    /// Maps an imported leaf name or alias to the root used path segment, e.g.
    /// `Clock -> solana_clock` for `use solana_clock::Clock`.
    pub import_origins: HashMap<String, String>,
    /// True when the file contains at least one glob `use` (`use foo::*;`),
    /// meaning names are in scope that seagrass cannot enumerate.
    /// Diagnostics that assert "name X does not exist" must suppress when this
    /// flag is set, unless the workspace index provides stronger evidence.
    pub has_glob_import: bool,
    /// True when a glob `use` can bring project-local names into scope. External
    /// preludes like `anchor_lang::prelude::*` are tracked by `has_glob_import`
    /// but do not make local absence claims ambiguous.
    pub has_local_glob_import: bool,
    /// Maps a `use Original as Alias` rename to the original terminal ident, so
    /// `Alias::CONST` resolves against `Original`'s associated values.
    pub import_aliases: HashMap<String, String>,
    pub value_items: Vec<NamedRange>,
    pub associated_value_items: HashMap<String, Vec<AssociatedValueRange>>,
    pub derived_init_space_types: HashSet<String>,
    pub instructions: Vec<InstructionSymbol>,
    pub functions: Vec<InstructionSymbol>,
    pub context_references: Vec<ContextReference>,
    pub declared_program_id: Option<DeclaredProgramId>,
    pub error_codes: Vec<ErrorCodeEnum>,
}

impl AnchorSymbols {
    fn from_items(items: &[Item]) -> Self {
        let mut symbols = Self::default();
        let local_module_names = items
            .iter()
            .filter_map(|item| {
                let Item::Mod(item_mod) = item else {
                    return None;
                };
                Some(item_mod.ident.to_string())
            })
            .collect::<HashSet<_>>();

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
                    collect_imported_names(
                        &item_use.tree,
                        &mut symbols.imported_names,
                        &mut symbols.import_aliases,
                        &mut symbols.import_origins,
                    );
                    if has_glob_in_use_tree(&item_use.tree) {
                        symbols.has_glob_import = true;
                    }
                    if has_local_glob_in_use_tree(&item_use.tree, &local_module_names) {
                        symbols.has_local_glob_import = true;
                    }
                }
                Item::Enum(item_enum) => {
                    symbols.enums.insert(
                        item_enum.ident.to_string(),
                        SymbolRange::from_enum(item_enum),
                    );
                    if let Some(error_code) = error_codes::error_code_enum(item_enum) {
                        symbols.error_codes.push(error_code);
                    }
                }
                Item::Impl(item_impl) => {
                    associated_values::collect_from_impl(
                        item_impl,
                        &mut symbols.associated_value_items,
                    );
                }
                Item::Macro(item_macro)
                    if path_last_is_any_ident(
                        &item_macro.mac.path,
                        idioms::PROGRAM_DECLARATION_MACROS,
                    ) =>
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

    /// Resolves a `use Original as Alias` rename back to `Original`. Returns the
    /// input unchanged when it is not an alias.
    pub fn resolve_type_alias<'a>(&'a self, type_name: &'a str) -> &'a str {
        self.import_aliases
            .get(type_name)
            .map(String::as_str)
            .unwrap_or(type_name)
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

#[derive(Debug, Clone)]
pub struct SymbolRange {
    pub name: String,
    pub range: Range,
    pub selection_range: Range,
    pub fields: Vec<SymbolRange>,
    pub variants: Vec<SymbolRange>,
    pub type_name: Option<String>,
    pub type_range: Option<Range>,
    /// Whitespace-normalized full Rust type text, retained for layout-sensitive
    /// users such as account-space estimation.
    pub type_signature: Option<String>,
    pub generic_type_names: Vec<String>,
    pub generic_type_ranges: Vec<NamedRange>,
    pub is_optional: bool,
    pub max_len_args: Vec<String>,
    pub account_constraints: Vec<AccountConstraint>,
    pub pda_constraint: Option<PdaConstraint>,
    pub instruction_arguments: Vec<InstructionAttributeArgument>,
    pub derive_attribute_range: Option<Range>,
    pub derive_accounts_range: Option<Range>,
    pub derive_init_space_range: Option<Range>,
    pub is_zero_copy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountFieldTypeSummary {
    pub type_name: Option<String>,
    pub generic_type_names: Vec<String>,
    pub is_optional: bool,
}

impl SymbolRange {
    fn from_struct(item_struct: &ItemStruct) -> Self {
        Self {
            name: item_struct.ident.to_string(),
            range: range_from_span(item_struct.span()),
            selection_range: range_from_span(item_struct.ident.span()),
            fields: field_symbols(item_struct),
            variants: Vec::new(),
            type_name: None,
            type_range: None,
            type_signature: None,
            generic_type_names: Vec::new(),
            generic_type_ranges: Vec::new(),
            is_optional: false,
            max_len_args: Vec::new(),
            account_constraints: Vec::new(),
            pda_constraint: None,
            instruction_arguments: instruction_attribute_arguments(&item_struct.attrs),
            derive_attribute_range: derive_attribute_range(&item_struct.attrs),
            derive_accounts_range: derive_accounts_range(&item_struct.attrs),
            derive_init_space_range: derive_init_space_range(&item_struct.attrs),
            is_zero_copy: is_zero_copy_struct(&item_struct.attrs),
        }
    }

    fn from_enum(item_enum: &ItemEnum) -> Self {
        Self {
            name: item_enum.ident.to_string(),
            range: range_from_span(item_enum.span()),
            selection_range: range_from_span(item_enum.ident.span()),
            fields: Vec::new(),
            variants: item_enum
                .variants
                .iter()
                .map(|variant| SymbolRange {
                    name: variant.ident.to_string(),
                    range: range_from_span(variant.span()),
                    selection_range: range_from_span(variant.ident.span()),
                    fields: symbols_from_fields(&variant.fields, &HashMap::new()),
                    variants: Vec::new(),
                    type_name: None,
                    type_range: None,
                    type_signature: None,
                    generic_type_names: Vec::new(),
                    generic_type_ranges: Vec::new(),
                    is_optional: false,
                    max_len_args: Vec::new(),
                    account_constraints: Vec::new(),
                    pda_constraint: None,
                    instruction_arguments: Vec::new(),
                    derive_attribute_range: None,
                    derive_accounts_range: None,
                    derive_init_space_range: None,
                    is_zero_copy: false,
                })
                .collect(),
            type_name: None,
            type_range: None,
            type_signature: None,
            generic_type_names: Vec::new(),
            generic_type_ranges: Vec::new(),
            is_optional: false,
            max_len_args: Vec::new(),
            account_constraints: Vec::new(),
            pda_constraint: None,
            instruction_arguments: Vec::new(),
            derive_attribute_range: derive_attribute_range(&item_enum.attrs),
            derive_accounts_range: None,
            derive_init_space_range: derive_init_space_range(&item_enum.attrs),
            is_zero_copy: false,
        }
    }
}

pub fn summarize_account_field_type(ty: &Type) -> AccountFieldTypeSummary {
    let (field_ty, is_optional) = account_field_type(ty);
    AccountFieldTypeSummary {
        type_name: type_name(field_ty),
        generic_type_names: generic_type_ranges(field_ty)
            .into_iter()
            .map(|range| range.name)
            .collect(),
        is_optional,
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
    /// Last path segment of the type (e.g. `Pubkey`, `Vec`) — used for the
    /// common scalar seed cases and member resolution.
    pub type_name: Option<String>,
    /// Whitespace-normalized full type (e.g. `Vec<u8>`, `[u8;32]`, `&[u8]`).
    /// Needed to distinguish byte containers that share a head segment.
    pub type_signature: Option<String>,
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
    pub source_account: String,
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

fn path_last_is_any_ident(path: &Path, names: &[&str]) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| idioms::ident_is_any(&segment.ident, names))
}

pub fn derives_accounts(attrs: &[Attribute]) -> bool {
    derive_accounts_range(attrs).is_some()
}

fn derive_accounts_range(attrs: &[Attribute]) -> Option<Range> {
    derive_path_range(attrs, "Accounts")
}

fn derive_init_space_range(attrs: &[Attribute]) -> Option<Range> {
    derive_path_range(attrs, "InitSpace")
}

fn derive_path_range(attrs: &[Attribute], name: &str) -> Option<Range> {
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
                .find(|path| path.is_ident(name))
                .map(|path| range_from_span(path.span()))
        })
}

fn derive_attribute_range(attrs: &[Attribute]) -> Option<Range> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("derive"))
        .map(|attr| range_from_span(attr.span()))
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
    let parser_pdas = parser_pda_constraints(item_struct);
    symbols_from_fields(&item_struct.fields, &parser_pdas)
}

fn symbols_from_fields(
    fields: &Fields,
    parser_pdas: &HashMap<String, PdaConstraint>,
) -> Vec<SymbolRange> {
    match fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let ident = field
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| index.to_string());
                field_symbol(&ident, field, parser_pdas)
            })
            .collect(),
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(index, field)| field_symbol(&index.to_string(), field, parser_pdas))
            .collect(),
        Fields::Unit => Vec::new(),
    }
}

fn field_symbol(
    field_name: &str,
    field: &syn::Field,
    parser_pdas: &HashMap<String, PdaConstraint>,
) -> SymbolRange {
    let (field_ty, is_optional) = account_field_type(&field.ty);
    let generic_type_ranges = generic_type_ranges(field_ty);
    let account_constraints = account_constraints(&field.attrs);
    let pda_constraint = parser_pdas.get(field_name).cloned().or_else(|| {
        account_constraints
            .iter()
            .find_map(|constraint| constraint.pda.clone())
    });
    SymbolRange {
        name: field_name.to_string(),
        range: range_from_span(field.span()),
        selection_range: field
            .ident
            .as_ref()
            .map(|ident| range_from_span(ident.span()))
            .unwrap_or_else(|| range_from_span(field.span())),
        fields: Vec::new(),
        variants: Vec::new(),
        type_name: type_name(field_ty),
        type_range: type_range(field_ty),
        type_signature: Some(normalize_token_text(
            &field.ty.to_token_stream().to_string(),
        )),
        generic_type_names: generic_type_ranges
            .iter()
            .map(|range| range.name.clone())
            .collect(),
        generic_type_ranges,
        is_optional,
        max_len_args: max_len_args(&field.attrs),
        account_constraints,
        pda_constraint,
        instruction_arguments: Vec::new(),
        derive_attribute_range: None,
        derive_accounts_range: None,
        derive_init_space_range: None,
        is_zero_copy: false,
    }
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

fn max_len_args(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("max_len"))
        .and_then(|attr| {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Expr, Token![,]>::parse_terminated,
            )
            .ok()
        })
        .map(|args| {
            args.into_iter()
                .map(|expr| normalize_token_text(&expr.to_token_stream().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn is_zero_copy_struct(attrs: &[Attribute]) -> bool {
    has_attr(attrs, "zero_copy")
        || attrs
            .iter()
            .filter(|attr| attr.path().is_ident("account"))
            .any(|attr| {
                normalize_token_text(&attr.meta.to_token_stream().to_string()).contains("zero_copy")
            })
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
                type_signature: Some(normalize_token_text(
                    &pat_type.ty.to_token_stream().to_string(),
                )),
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
