use {
    super::common::{diagnostic_quickfix, single_document_edit, single_text_edit},
    crate::{document::ParsedDocument, range::range_from_span},
    quote::ToTokens,
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
    },
    tower_lsp::lsp_types::{CodeAction, CodeActionKind, Diagnostic, Range, TextEdit, Url},
};

const CHECKED_ARITHMETIC_QUICKFIX: &str = "checked-arithmetic";
const ANCHOR_PROGRAM_KIND: &str = "anchor";
const ANCHOR_PROGRAM_ERROR_PATH: &str = "anchor_lang::solana_program::program_error::ProgramError";
const SOLANA_PROGRAM_ERROR_PATH: &str = "solana_program::program_error::ProgramError";

pub fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    _range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_quickfix(diagnostic) == Some(CHECKED_ARITHMETIC_QUICKFIX))
        .filter_map(|diagnostic| {
            let edit = checked_arithmetic_edit(document, diagnostic)?;
            Some(CodeAction {
                title: "Use checked arithmetic".to_string(),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "checked-arithmetic",
                })),
            })
        })
        .collect()
}

fn checked_arithmetic_edit(document: &ParsedDocument, diagnostic: &Diagnostic) -> Option<TextEdit> {
    let replacement =
        ArithmeticReplacement::for_operator_range(document.syntax(), diagnostic.range)?;
    Some(single_text_edit(
        replacement.range,
        replacement.render(program_error_path(diagnostic)),
    ))
}

fn program_error_path(diagnostic: &Diagnostic) -> &'static str {
    if diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("programKind"))
        .and_then(|value| value.as_str())
        == Some(ANCHOR_PROGRAM_KIND)
    {
        ANCHOR_PROGRAM_ERROR_PATH
    } else {
        SOLANA_PROGRAM_ERROR_PATH
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArithmeticReplacement {
    range: Range,
    left: String,
    right: String,
    method: &'static str,
    assignment: bool,
}

impl ArithmeticReplacement {
    fn for_operator_range(syntax: &syn::File, operator_range: Range) -> Option<Self> {
        let mut visitor = ArithmeticReplacementVisitor {
            operator_range,
            replacement: None,
        };
        visitor.visit_file(syntax);
        visitor.replacement
    }

    fn render(&self, program_error_path: &str) -> String {
        let checked = format!(
            "{}.{}({}).ok_or({program_error_path}::ArithmeticOverflow)?",
            self.left, self.method, self.right
        );
        if self.assignment {
            format!("{} = {checked}", self.left)
        } else {
            checked
        }
    }
}

struct ArithmeticReplacementVisitor {
    operator_range: Range,
    replacement: Option<ArithmeticReplacement>,
}

impl<'ast> Visit<'ast> for ArithmeticReplacementVisitor {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if self.replacement.is_some() {
            return;
        }
        if range_from_span(node.op.span()) == self.operator_range {
            self.replacement = replacement_for_binary(node);
            return;
        }
        visit::visit_expr_binary(self, node);
    }
}

fn replacement_for_binary(node: &syn::ExprBinary) -> Option<ArithmeticReplacement> {
    if operand_needs_skip(&node.left) || operand_needs_skip(&node.right) {
        return None;
    }
    let (method, assignment) = checked_method(&node.op)?;
    Some(ArithmeticReplacement {
        range: range_from_span(node.span()),
        left: expression_text(&node.left),
        right: expression_text(&node.right),
        method,
        assignment,
    })
}

fn checked_method(op: &syn::BinOp) -> Option<(&'static str, bool)> {
    match op {
        syn::BinOp::Add(_) => Some(("checked_add", false)),
        syn::BinOp::Sub(_) => Some(("checked_sub", false)),
        syn::BinOp::Mul(_) => Some(("checked_mul", false)),
        syn::BinOp::Div(_) => Some(("checked_div", false)),
        syn::BinOp::Rem(_) => Some(("checked_rem", false)),
        syn::BinOp::AddAssign(_) => Some(("checked_add", true)),
        syn::BinOp::SubAssign(_) => Some(("checked_sub", true)),
        syn::BinOp::MulAssign(_) => Some(("checked_mul", true)),
        syn::BinOp::DivAssign(_) => Some(("checked_div", true)),
        syn::BinOp::RemAssign(_) => Some(("checked_rem", true)),
        _ => None,
    }
}

fn operand_needs_skip(expr: &syn::Expr) -> bool {
    let mut visitor = UnsafeOperandVisitor::default();
    visitor.visit_expr(expr);
    visitor.skip
}

#[derive(Default)]
struct UnsafeOperandVisitor {
    skip: bool,
}

impl<'ast> Visit<'ast> for UnsafeOperandVisitor {
    fn visit_expr_method_call(&mut self, _node: &'ast syn::ExprMethodCall) {
        self.skip = true;
    }

    fn visit_expr_try(&mut self, _node: &'ast syn::ExprTry) {
        self.skip = true;
    }
}

fn expression_text(expr: &syn::Expr) -> String {
    expr.to_token_stream().to_string()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::document::ParsedDocument,
        serde_json::json,
        syn::visit::Visit,
        tower_lsp::lsp_types::{NumberOrString, Position},
    };

    #[test]
    fn checked_arithmetic_actions_parse_for_binary_operators() {
        for (operator, method) in [
            ("+", "checked_add"),
            ("-", "checked_sub"),
            ("*", "checked_mul"),
            ("/", "checked_div"),
            ("%", "checked_rem"),
        ] {
            let source = format!(
                r#"
fn process(amount: u64, delta: u64) -> Result<(), solana_program::program_error::ProgramError> {{
    let total = amount {operator} delta;
    Ok(())
}}
"#
            );
            let updated = apply_first_action(&source, operator);
            assert!(
                updated.contains(method),
                "missing {method} replacement in {updated}"
            );
            ParsedDocument::parse(updated).unwrap();
        }
    }

    #[test]
    fn checked_arithmetic_actions_parse_for_compound_assignment() {
        let source = r#"
fn process(mut amount: u64, delta: u64) -> Result<(), solana_program::program_error::ProgramError> {
    amount += delta;
    Ok(())
}
"#;
        let updated = apply_first_action(source, "+=");

        assert!(updated.contains("amount = amount.checked_add(delta)"));
        ParsedDocument::parse(updated).unwrap();
    }

    #[test]
    fn checked_arithmetic_action_uses_anchor_program_error_path() {
        let source = r#"
fn process(amount: u64, delta: u64) -> Result<()> {
    let total = amount + delta;
    Ok(())
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostic = diagnostic_for(source, "+", Some(ANCHOR_PROGRAM_KIND));
        let action = code_actions(
            &document,
            Url::parse("file:///tmp/lib.rs").unwrap(),
            diagnostic.range,
            &[diagnostic],
        )
        .pop()
        .unwrap();
        let text = first_edit(&action).new_text.clone();

        assert!(text.contains(ANCHOR_PROGRAM_ERROR_PATH));
    }

    #[test]
    fn checked_arithmetic_skips_method_call_operands() {
        let source = r#"
fn process(amount: u64, delta: u64) -> Result<(), solana_program::program_error::ProgramError> {
    let total = amount.saturating_add(1) + delta;
    Ok(())
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostic = diagnostic_for(source, "+", None);

        assert!(code_actions(
            &document,
            Url::parse("file:///tmp/lib.rs").unwrap(),
            diagnostic.range,
            &[diagnostic],
        )
        .is_empty());
    }

    #[test]
    fn checked_arithmetic_action_only_uses_own_diagnostic() {
        let source = r#"
fn process(amount: u64, delta: u64) -> Result<(), solana_program::program_error::ProgramError> {
    let total = amount + delta;
    Ok(())
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let mut diagnostic = diagnostic_for(source, "+", None);
        diagnostic.data = Some(json!({ "quickfix": "other-action" }));

        assert!(code_actions(
            &document,
            Url::parse("file:///tmp/lib.rs").unwrap(),
            diagnostic.range,
            &[diagnostic],
        )
        .is_empty());
    }

    fn apply_first_action(source: &str, operator_needle: &str) -> String {
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostic = diagnostic_for(source, operator_needle, None);
        let action = code_actions(
            &document,
            Url::parse("file:///tmp/lib.rs").unwrap(),
            diagnostic.range,
            &[diagnostic],
        )
        .pop()
        .unwrap();
        apply_edit(source, first_edit(&action))
    }

    fn diagnostic_for(
        source: &str,
        operator_needle: &str,
        program_kind: Option<&str>,
    ) -> Diagnostic {
        let range = range_for_operator(source, operator_needle);
        Diagnostic {
            range,
            severity: None,
            code: Some(NumberOrString::String("solana-code-quality".to_string())),
            code_description: None,
            source: Some(crate::diagnostics::SOURCE.to_string()),
            message: "Use checked arithmetic".to_string(),
            related_information: None,
            tags: None,
            data: Some(json!({
                "quickfix": CHECKED_ARITHMETIC_QUICKFIX,
                "programKind": program_kind.unwrap_or("native-solana"),
            })),
        }
    }

    fn first_edit(action: &CodeAction) -> &TextEdit {
        action
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .first()
            .unwrap()
    }

    fn apply_edit(source: &str, edit: &TextEdit) -> String {
        let start = byte_offset(source, edit.range.start);
        let end = byte_offset(source, edit.range.end);
        format!("{}{}{}", &source[..start], edit.new_text, &source[end..])
    }

    fn range_for_operator(source: &str, operator: &str) -> Range {
        let document = ParsedDocument::parse(source).unwrap();
        let mut visitor = TestOperatorRangeVisitor {
            operator,
            range: None,
        };
        visitor.visit_file(document.syntax());
        visitor.range.unwrap()
    }

    struct TestOperatorRangeVisitor<'a> {
        operator: &'a str,
        range: Option<Range>,
    }

    impl<'ast> Visit<'ast> for TestOperatorRangeVisitor<'_> {
        fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
            if self.range.is_some() {
                return;
            }
            if test_operator_text(&node.op) == Some(self.operator) {
                self.range = Some(range_from_span(node.op.span()));
                return;
            }
            syn::visit::visit_expr_binary(self, node);
        }
    }

    fn test_operator_text(op: &syn::BinOp) -> Option<&'static str> {
        match op {
            syn::BinOp::Add(_) => Some("+"),
            syn::BinOp::Sub(_) => Some("-"),
            syn::BinOp::Mul(_) => Some("*"),
            syn::BinOp::Div(_) => Some("/"),
            syn::BinOp::Rem(_) => Some("%"),
            syn::BinOp::AddAssign(_) => Some("+="),
            syn::BinOp::SubAssign(_) => Some("-="),
            syn::BinOp::MulAssign(_) => Some("*="),
            syn::BinOp::DivAssign(_) => Some("/="),
            syn::BinOp::RemAssign(_) => Some("%="),
            _ => None,
        }
    }

    fn byte_offset(source: &str, position: Position) -> usize {
        let mut offset = 0usize;
        for (line_index, line) in source.split_inclusive('\n').enumerate() {
            if line_index == position.line as usize {
                return offset + position.character as usize;
            }
            offset += line.len();
        }
        source.len()
    }
}
