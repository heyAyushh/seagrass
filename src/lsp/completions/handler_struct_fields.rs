use {
    crate::{account_members, document::ParsedDocument, workspace::WorkspaceIndex},
    std::collections::HashSet,
    tower_lsp::lsp_types::{
        CompletionItem, CompletionItemKind, CompletionTextEdit, Position, TextEdit,
    },
};

const STRUCT_FIELD_COMPLETION_KIND: &str = "handler-struct-literal-field";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StructLiteralFieldContext {
    pub(super) type_name: String,
    pub(super) prefix: String,
    used_fields: Vec<String>,
}

pub(super) fn context_at(source: &str, offset: usize) -> Option<StructLiteralFieldContext> {
    let offset = offset.min(source.len());
    let open_brace = last_unclosed_brace_before(source, offset)?;
    let type_name = struct_type_before_open_brace(source, open_brace)?;
    let body = source.get(open_brace + '{'.len_utf8()..offset)?;
    let fields = StructLiteralFieldSegments::from_body(body)?;
    let prefix = fields.current_prefix()?;

    Some(StructLiteralFieldContext {
        type_name,
        prefix,
        used_fields: fields.used_field_names(),
    })
}

pub(super) fn completions(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
    context: &StructLiteralFieldContext,
) -> Option<Vec<CompletionItem>> {
    let used_fields = context
        .used_fields
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let replacement_range = super::prefix_replacement_range(position, &context.prefix);
    let mut items =
        account_members::resolved_struct_members(document, workspace_index, &context.type_name)?
            .members
            .iter()
            .filter(|member| member.completion_kind == CompletionItemKind::FIELD)
            .filter(|member| super::matches_completion_prefix(&member.name, &context.prefix))
            .filter(|member| !used_fields.contains(member.name.as_str()))
            .map(|member| CompletionItem {
                label: member.name.clone(),
                kind: Some(CompletionItemKind::FIELD),
                detail: Some(format!("Struct field in `{}`", context.type_name)),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: replacement_range,
                    new_text: member.name.clone(),
                })),
                sort_text: Some(format!(
                    "000_{STRUCT_FIELD_COMPLETION_KIND}_{}",
                    member.name
                )),
                data: Some(serde_json::json!({
                    "anchorCompletion": STRUCT_FIELD_COMPLETION_KIND,
                    "ownerType": context.type_name,
                    "field": member.name,
                    "fieldType": member.detail,
                })),
                ..CompletionItem::default()
            })
            .collect::<Vec<_>>();

    items.sort_by(|left, right| left.label.cmp(&right.label));
    (!items.is_empty()).then_some(items)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ScanState {
    #[default]
    Code,
    LineComment,
    BlockComment,
    String(char),
}

#[derive(Debug)]
struct StructLiteralFieldSegments<'a> {
    completed: Vec<&'a str>,
    current: &'a str,
}

impl<'a> StructLiteralFieldSegments<'a> {
    fn from_body(body: &'a str) -> Option<Self> {
        let mut completed = Vec::new();
        let mut segment_start = 0usize;
        let mut scanner = TopLevelScanner::default();

        for (idx, ch) in body.char_indices() {
            if scanner.observe(ch) == TopLevelObservation::FieldSeparator {
                completed.push(body.get(segment_start..idx)?);
                segment_start = idx + ch.len_utf8();
            }
        }

        Some(Self {
            completed,
            current: body.get(segment_start..)?,
        })
    }

    fn current_prefix(&self) -> Option<String> {
        if has_top_level_colon(self.current) {
            return None;
        }
        let current = self.current.trim_start();
        if current.starts_with("..") {
            return None;
        }
        let prefix = current
            .rsplit(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .next()
            .unwrap_or_default();
        Some(prefix.to_string())
    }

    fn used_field_names(&self) -> Vec<String> {
        let mut names = self
            .completed
            .iter()
            .filter_map(|segment| literal_field_name(segment))
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }
}

#[derive(Debug, Default)]
struct TopLevelScanner {
    paren_depth: usize,
    bracket_depth: usize,
    brace_depth: usize,
    state: ScanState,
    escaped: bool,
    previous: Option<char>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TopLevelObservation {
    None,
    FieldSeparator,
}

impl TopLevelScanner {
    fn observe(&mut self, ch: char) -> TopLevelObservation {
        match self.state {
            ScanState::LineComment => {
                if ch == '\n' {
                    self.state = ScanState::Code;
                }
                self.previous = Some(ch);
                return TopLevelObservation::None;
            }
            ScanState::BlockComment => {
                if self.previous == Some('*') && ch == '/' {
                    self.state = ScanState::Code;
                }
                self.previous = Some(ch);
                return TopLevelObservation::None;
            }
            ScanState::String(quote) => {
                if self.escaped {
                    self.escaped = false;
                } else if ch == '\\' {
                    self.escaped = true;
                } else if ch == quote {
                    self.state = ScanState::Code;
                }
                self.previous = Some(ch);
                return TopLevelObservation::None;
            }
            ScanState::Code => {}
        }

        if self.previous == Some('/') && ch == '/' {
            self.state = ScanState::LineComment;
            self.previous = Some(ch);
            return TopLevelObservation::None;
        }
        if self.previous == Some('/') && ch == '*' {
            self.state = ScanState::BlockComment;
            self.previous = Some(ch);
            return TopLevelObservation::None;
        }

        let observation = match ch {
            '"' => {
                self.state = ScanState::String(ch);
                TopLevelObservation::None
            }
            '(' => {
                self.paren_depth += 1;
                TopLevelObservation::None
            }
            ')' => {
                self.paren_depth = self.paren_depth.saturating_sub(1);
                TopLevelObservation::None
            }
            '[' => {
                self.bracket_depth += 1;
                TopLevelObservation::None
            }
            ']' => {
                self.bracket_depth = self.bracket_depth.saturating_sub(1);
                TopLevelObservation::None
            }
            '{' => {
                self.brace_depth += 1;
                TopLevelObservation::None
            }
            '}' => {
                self.brace_depth = self.brace_depth.saturating_sub(1);
                TopLevelObservation::None
            }
            ',' if self.is_top_level() => TopLevelObservation::FieldSeparator,
            _ => TopLevelObservation::None,
        };
        self.previous = Some(ch);
        observation
    }

    fn is_top_level(&self) -> bool {
        self.paren_depth == 0 && self.bracket_depth == 0 && self.brace_depth == 0
    }
}

fn last_unclosed_brace_before(source: &str, offset: usize) -> Option<usize> {
    let mut stack = Vec::new();
    let mut state = ScanState::Code;
    let mut escaped = false;
    let mut previous = None;

    for (idx, ch) in source[..offset].char_indices() {
        match state {
            ScanState::LineComment => {
                if ch == '\n' {
                    state = ScanState::Code;
                }
                previous = Some(ch);
                continue;
            }
            ScanState::BlockComment => {
                if previous == Some('*') && ch == '/' {
                    state = ScanState::Code;
                }
                previous = Some(ch);
                continue;
            }
            ScanState::String(quote) => {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    state = ScanState::Code;
                }
                previous = Some(ch);
                continue;
            }
            ScanState::Code => {}
        }

        if previous == Some('/') && ch == '/' {
            state = ScanState::LineComment;
            previous = Some(ch);
            continue;
        }
        if previous == Some('/') && ch == '*' {
            state = ScanState::BlockComment;
            previous = Some(ch);
            continue;
        }

        match ch {
            '"' => state = ScanState::String(ch),
            '{' => stack.push(idx),
            '}' => {
                stack.pop();
            }
            _ => {}
        }
        previous = Some(ch);
    }

    stack.into_iter().next_back()
}

fn struct_type_before_open_brace(source: &str, open_brace: usize) -> Option<String> {
    let before = source.get(..open_brace)?.trim_end();
    let path_start = before
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| {
            (!(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')).then_some(idx + ch.len_utf8())
        })
        .unwrap_or(0);
    let path = before.get(path_start..)?.trim_start_matches("::");
    let type_name = path.rsplit("::").next()?.trim();
    (crate::syntax::is_ascii_type_identifier(type_name) && path_segments_are_identifiers(path))
        .then(|| type_name.to_string())
}

fn path_segments_are_identifiers(path: &str) -> bool {
    path.split("::").all(crate::syntax::is_ascii_identifier)
}

fn literal_field_name(segment: &str) -> Option<String> {
    let segment = segment.trim_start();
    if segment.starts_with("..") {
        return None;
    }
    let field_len = segment
        .char_indices()
        .find_map(|(idx, ch)| (!(ch.is_ascii_alphanumeric() || ch == '_')).then_some(idx))
        .unwrap_or(segment.len());
    let field = segment.get(..field_len)?;
    if field.is_empty() {
        return None;
    }
    Some(field.to_string())
}

fn has_top_level_colon(segment: &str) -> bool {
    let mut scanner = TopLevelScanner::default();
    for ch in segment.chars() {
        if ch == ':' && scanner.is_top_level() {
            return true;
        }
        scanner.observe(ch);
    }
    false
}
