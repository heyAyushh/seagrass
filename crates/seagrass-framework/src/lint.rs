use {
    crate::{
        diagnostics::FrameworkDocument,
        range::{byte_offset_at, range_from_span},
    },
    std::marker::PhantomData,
    syn::{
        punctuated::Punctuated,
        spanned::Spanned,
        token::Comma,
        visit::{self, Visit},
    },
    tower_lsp::lsp_types::{Diagnostic, Position, Range},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    InstructionBody,
    HelperFnBody,
    AccountsStructField,
    AttributeArguments,
    DocComment,
    StringLiteral,
    UseItem,
    ItemDecl,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    Heuristic,
    Derived,
    Authoritative,
}

impl Confidence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Heuristic => "heuristic",
            Self::Derived => "derived",
            Self::Authoritative => "authoritative",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}

impl Applicability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MachineApplicable => "MachineApplicable",
            Self::MaybeIncorrect => "MaybeIncorrect",
            Self::HasPlaceholders => "HasPlaceholders",
            Self::Unspecified => "Unspecified",
        }
    }
}

pub trait LintVisitor<'ast>: Visit<'ast> {
    const SCOPE: &'static [Region];
    const CONFIDENCE: Confidence;
    const APPLICABILITY: Applicability;
    const TOPIC: &'static str;

    fn finish(self) -> Vec<Diagnostic>;
}

pub struct FunctionBody<'ast> {
    pub inputs: &'ast Punctuated<syn::FnArg, Comma>,
}

pub fn run_lint_visitor<'ast, V>(
    document: FrameworkDocument<'ast>,
    mut visitor: V,
) -> Vec<Diagnostic>
where
    V: LintVisitor<'ast>,
{
    if document.syntax().items.is_empty() {
        return Vec::new();
    }

    visitor.visit_file(document.syntax());
    diagnostics_in_scope::<V>(document, visitor.finish())
}

pub fn run_lint_visitor_on_functions<'ast, V, F>(
    document: FrameworkDocument<'ast>,
    mut visitor_for_function: F,
) -> Vec<Diagnostic>
where
    V: LintVisitor<'ast>,
    F: FnMut(FunctionBody<'ast>) -> V,
{
    if document.syntax().items.is_empty() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    let mut runner = FunctionBodyRunner::<V, F> {
        visitor_for_function: &mut visitor_for_function,
        diagnostics: &mut diagnostics,
        visitor_marker: PhantomData,
    };
    runner.visit_file(document.syntax());
    diagnostics_in_scope::<V>(document, diagnostics)
}

struct FunctionBodyRunner<'a, 'ast, V, F>
where
    V: LintVisitor<'ast>,
    F: FnMut(FunctionBody<'ast>) -> V,
{
    visitor_for_function: &'a mut F,
    diagnostics: &'a mut Vec<Diagnostic>,
    visitor_marker: PhantomData<(&'ast (), V)>,
}

impl<'ast, V, F> FunctionBodyRunner<'_, 'ast, V, F>
where
    V: LintVisitor<'ast>,
    F: FnMut(FunctionBody<'ast>) -> V,
{
    fn collect(&mut self, body: FunctionBody<'ast>, visit: impl FnOnce(&mut V)) {
        let mut visitor = (self.visitor_for_function)(body);
        visit(&mut visitor);
        self.diagnostics.extend(visitor.finish());
    }
}

impl<'ast, V, F> Visit<'ast> for FunctionBodyRunner<'_, 'ast, V, F>
where
    V: LintVisitor<'ast>,
    F: FnMut(FunctionBody<'ast>) -> V,
{
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.collect(
            FunctionBody {
                inputs: &node.sig.inputs,
            },
            |visitor| visitor.visit_item_fn(node),
        );
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.collect(
            FunctionBody {
                inputs: &node.sig.inputs,
            },
            |visitor| visitor.visit_impl_item_fn(node),
        );
    }
}

fn diagnostics_in_scope<'ast, V>(
    document: FrameworkDocument<'ast>,
    diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic>
where
    V: LintVisitor<'ast>,
{
    let regions = RegionMap::from_document(document);
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            byte_offset_for_position(document.source(), diagnostic.range.start).is_some_and(
                |byte_offset| {
                    let region = regions.region_at(byte_offset);
                    V::SCOPE.contains(&region)
                },
            )
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct RegionMap {
    spans: Vec<RegionSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegionSpan {
    start: usize,
    end: usize,
    region: Region,
}

impl RegionMap {
    pub fn from_document(document: FrameworkDocument<'_>) -> Self {
        let mut visitor = RegionVisitor {
            source: document.source(),
            in_program_module: false,
            spans: Vec::new(),
        };
        visitor.visit_file(document.syntax());
        Self {
            spans: visitor.spans,
        }
    }

    pub fn region_at(&self, byte_offset: usize) -> Region {
        self.spans
            .iter()
            .filter(|span| span.contains(byte_offset))
            .min_by_key(|span| span.len())
            .map(|span| span.region)
            .unwrap_or(Region::Other)
    }

    pub fn allows_executable_lints(&self, byte_offset: usize) -> bool {
        matches!(
            self.region_at(byte_offset),
            Region::InstructionBody | Region::HelperFnBody
        )
    }
}

impl RegionSpan {
    fn contains(self, byte_offset: usize) -> bool {
        self.start <= byte_offset && byte_offset < self.end
    }

    fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

struct RegionVisitor<'a> {
    source: &'a str,
    in_program_module: bool,
    spans: Vec<RegionSpan>,
}

impl RegionVisitor<'_> {
    fn push_span(&mut self, range: Range, region: Region) {
        let Some(start) = byte_offset_at(self.source, range.start) else {
            return;
        };
        let Some(end) = byte_offset_at(self.source, range.end) else {
            return;
        };
        if start < end {
            self.spans.push(RegionSpan { start, end, region });
        }
    }

    fn push_spanned(&mut self, span: proc_macro2::Span, region: Region) {
        self.push_span(range_from_span(span), region);
    }
}

impl<'ast> Visit<'ast> for RegionVisitor<'_> {
    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        if node.path().is_ident("doc") {
            self.push_spanned(node.span(), Region::DocComment);
        } else {
            self.push_spanned(node.span(), Region::AttributeArguments);
        }
        visit::visit_attribute(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.push_spanned(node.span(), Region::ItemDecl);
        self.push_spanned(
            node.block.span(),
            if self.in_program_module {
                Region::InstructionBody
            } else {
                Region::HelperFnBody
            },
        );
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.push_spanned(node.sig.span(), Region::ItemDecl);
        self.push_spanned(node.block.span(), Region::HelperFnBody);
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let was_in_program_module = self.in_program_module;
        self.in_program_module |= node
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("program"));
        self.push_spanned(node.span(), Region::ItemDecl);
        visit::visit_item_mod(self, node);
        self.in_program_module = was_in_program_module;
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        self.push_spanned(node.span(), Region::ItemDecl);
        if derives_accounts(node) {
            node.fields
                .iter()
                .for_each(|field| self.push_spanned(field.span(), Region::AccountsStructField));
        }
        visit::visit_item_struct(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        self.push_spanned(node.span(), Region::UseItem);
        visit::visit_item_use(self, node);
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        self.push_spanned(node.span(), Region::StringLiteral);
        visit::visit_lit_str(self, node);
    }
}

fn derives_accounts(item_struct: &syn::ItemStruct) -> bool {
    item_struct.attrs.iter().any(|attr| {
        let syn::Meta::List(list) = &attr.meta else {
            return false;
        };
        attr.path().is_ident("derive")
            && list
                .tokens
                .to_string()
                .split(',')
                .any(|name| name.split_whitespace().any(|segment| segment == "Accounts"))
    })
}

pub fn byte_offset_for_position(source: &str, position: Position) -> Option<usize> {
    byte_offset_at(source, position)
}
