use {
    super::{CursorContext, CursorContextKind},
    tower_lsp::lsp_types::CompletionItem,
};

const PENDING_FIX_SCORE: u16 = 0;
const TYPE_MATCH_SCORE: u16 = 10;
const EXACT_PREFIX_SCORE: u16 = 15;
const PRESELECT_SCORE: u16 = 20;
const PREFIX_SCORE: u16 = 30;
const DEFAULT_SCORE: u16 = 50;

pub(super) struct RankingContext<'a> {
    cursor_context: &'a CursorContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RelevanceScore {
    value: u16,
    reason: &'static str,
}

impl<'a> RankingContext<'a> {
    pub(super) fn new(cursor_context: &'a CursorContext) -> Self {
        Self { cursor_context }
    }

    fn typed_prefix(&self) -> Option<&str> {
        match self.cursor_context.kind() {
            CursorContextKind::AccountPath { prefix }
            | CursorContextKind::ContextType { prefix }
            | CursorContextKind::AccountConstraintValue { prefix }
            | CursorContextKind::InstructionAttribute { prefix }
            | CursorContextKind::AccountsField { prefix }
            | CursorContextKind::HandlerValue { prefix }
            | CursorContextKind::HandlerMember { prefix } => Some(prefix.as_str()),
            CursorContextKind::AccountConstraintKey { context } => Some(context.prefix.as_str()),
            CursorContextKind::HandlerStructField { context } => Some(context.prefix.as_str()),
            CursorContextKind::NotAnchor => None,
        }
        .filter(|prefix| !prefix.is_empty())
    }
}

pub(super) fn rank_completion_items(context: &RankingContext<'_>, items: &mut [CompletionItem]) {
    for (index, item) in items.iter_mut().enumerate() {
        let original_sort = item
            .sort_text
            .take()
            .unwrap_or_else(|| format!("999_seagrass_{}", item.label));
        let rank = relevance_score(context, item);
        item.sort_text = Some(format!(
            "{:03}_seagrass_rank_{}_{index:04}_{original_sort}",
            rank.value, rank.reason
        ));
        if rank.value <= TYPE_MATCH_SCORE {
            item.preselect = Some(true);
        }
    }
}

pub(super) fn set_data_bool(item: &mut CompletionItem, key: &str, value: bool) {
    let data = item
        .data
        .take()
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let mut object = match data {
        serde_json::Value::Object(object) => object,
        other => {
            item.data = Some(other);
            return;
        }
    };
    object.insert(key.to_string(), serde_json::Value::Bool(value));
    item.data = Some(serde_json::Value::Object(object));
}

pub(super) fn relevance_score(
    context: &RankingContext<'_>,
    item: &CompletionItem,
) -> RelevanceScore {
    if data_bool(item, "pendingFixTarget") {
        return RelevanceScore {
            value: PENDING_FIX_SCORE,
            reason: "pending_fix",
        };
    }
    if data_bool(item, "typeMatched") {
        return RelevanceScore {
            value: TYPE_MATCH_SCORE,
            reason: "type_match",
        };
    }
    if let Some(prefix) = context.typed_prefix() {
        if completion_label_or_insert_text(item).any(|candidate| normalized_eq(candidate, prefix)) {
            return RelevanceScore {
                value: EXACT_PREFIX_SCORE,
                reason: "exact_prefix",
            };
        }
    }
    if item.preselect == Some(true) {
        return RelevanceScore {
            value: PRESELECT_SCORE,
            reason: "preselect",
        };
    }
    if let Some(prefix) = context.typed_prefix() {
        if completion_label_or_insert_text(item)
            .any(|candidate| normalized_starts(candidate, prefix))
        {
            return RelevanceScore {
                value: PREFIX_SCORE,
                reason: "prefix",
            };
        }
    }
    RelevanceScore {
        value: DEFAULT_SCORE,
        reason: "default",
    }
}

fn data_bool(item: &CompletionItem, key: &str) -> bool {
    item.data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn completion_label_or_insert_text(item: &CompletionItem) -> impl Iterator<Item = &str> {
    std::iter::once(item.label.as_str()).chain(item.insert_text.as_deref())
}

fn normalized_eq(candidate: &str, prefix: &str) -> bool {
    normalize_candidate(candidate) == normalize_candidate(prefix)
}

fn normalized_starts(candidate: &str, prefix: &str) -> bool {
    normalize_candidate(candidate).starts_with(&normalize_candidate(prefix))
}

fn normalize_candidate(value: &str) -> String {
    value
        .trim_start_matches('_')
        .trim_end_matches(" =")
        .to_ascii_lowercase()
}
