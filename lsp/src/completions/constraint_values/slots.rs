use {
    crate::{
        constraint_catalog::{self, ConstraintSpec, ConstraintValueKind},
        document::{AccountAttributeCursor, AccountAttributeSlot},
    },
    tower_lsp::lsp_types::{CompletionItem, Position},
};

const CONSTRAINT_KEY_DATA_FIELD: &str = "constraintKey";
const CONSTRAINT_VALUE_KIND_DATA_FIELD: &str = "constraintValueKind";

pub(super) struct ConstraintValueContext {
    pub(super) slot: ConstraintValueSlot,
    pub(super) prefix: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ConstraintValueSlot {
    pub(super) key: &'static str,
    pub(super) value_kind: ConstraintValueKind,
}

impl ConstraintValueSlot {
    fn from_spec(spec: &'static ConstraintSpec) -> Option<Self> {
        Some(Self {
            key: constraint_catalog::key(spec.label),
            value_kind: spec.value_kind,
        })
    }
}

pub(super) fn constraint_value_context_for_cursor(
    source: &str,
    position: Position,
    cursor: Option<AccountAttributeCursor>,
) -> Option<ConstraintValueContext> {
    let cursor = cursor.or_else(|| AccountAttributeCursor::from_source(source, position))?;
    if cursor.slot != AccountAttributeSlot::Value {
        return None;
    }
    let key = cursor.constraint_key.as_deref()?;
    let slot = constraint_catalog::CONSTRAINTS
        .iter()
        .filter(|spec| spec.label.ends_with(" ="))
        .find(|spec| constraint_catalog::key(spec.label) == key)
        .and_then(ConstraintValueSlot::from_spec)?;

    Some(ConstraintValueContext {
        slot,
        prefix: cursor.prefix,
    })
}

pub(super) fn attach_slot_data(
    mut item: CompletionItem,
    slot: ConstraintValueSlot,
) -> CompletionItem {
    let data = item
        .data
        .take()
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let mut object = match data {
        serde_json::Value::Object(object) => object,
        value => {
            item.data = Some(value);
            return item;
        }
    };
    object.insert(
        CONSTRAINT_KEY_DATA_FIELD.to_string(),
        serde_json::Value::String(slot.key.to_string()),
    );
    object.insert(
        CONSTRAINT_VALUE_KIND_DATA_FIELD.to_string(),
        serde_json::Value::String(constraint_value_kind_name(slot.value_kind).to_string()),
    );
    item.data = Some(serde_json::Value::Object(object));
    item
}

pub(super) fn completion_matches_value_prefix(item: &CompletionItem, prefix: &str) -> bool {
    let normalized_prefix = prefix.trim_start_matches('_').to_ascii_lowercase();
    item.label
        .to_ascii_lowercase()
        .starts_with(&normalized_prefix)
        || item
            .label
            .trim_start_matches('_')
            .to_ascii_lowercase()
            .starts_with(&normalized_prefix)
        || item.insert_text.as_deref().is_some_and(|text| {
            text.to_ascii_lowercase().starts_with(&normalized_prefix)
                || text
                    .trim_start_matches('_')
                    .to_ascii_lowercase()
                    .starts_with(&normalized_prefix)
        })
}

fn constraint_value_kind_name(value_kind: ConstraintValueKind) -> &'static str {
    match value_kind {
        ConstraintValueKind::None => "none",
        ConstraintValueKind::AccountReference => "account-reference",
        ConstraintValueKind::SignerReference => "signer-reference",
        ConstraintValueKind::ProgramReference => "program-reference",
        ConstraintValueKind::InstructionArgument => "instruction-argument",
        ConstraintValueKind::Boolean => "boolean",
        ConstraintValueKind::Keyword => "keyword",
        ConstraintValueKind::Space => "space",
        ConstraintValueKind::Seeds => "seeds",
        ConstraintValueKind::AnyExpression => "any-expression",
    }
}
