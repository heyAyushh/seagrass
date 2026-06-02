use {
    super::{diagnostic_from_range, EVIDENCE_SOURCE, TOPIC},
    crate::{
        account_members,
        document::ParsedDocument,
        lsp::diagnostics::{
            lint::{Applicability, Confidence},
            registry::AnchorDiagnosticKind,
        },
        workspace::WorkspaceIndex,
    },
    std::collections::HashSet,
    tower_lsp::lsp_types::{CompletionItemKind, Diagnostic, Range},
};

const UNKNOWN_METHOD_REASON: &str = "unknown-handler-method";
const FIELD_CALLED_AS_METHOD_REASON: &str = "field-called-as-method";

pub(super) fn method_call_diagnostic(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    range: Range,
    receiver_path: &str,
    receiver_type: &str,
    method_name: &str,
    can_remove_call: bool,
) -> Option<Diagnostic> {
    field_called_as_method_diagnostic(
        document,
        workspace_index,
        range,
        receiver_path,
        receiver_type,
        method_name,
        can_remove_call,
    )
    .or_else(|| {
        unknown_method_diagnostic(
            document,
            workspace_index,
            range,
            receiver_path,
            receiver_type,
            method_name,
        )
    })
}

fn field_called_as_method_diagnostic(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    range: Range,
    receiver_path: &str,
    receiver_type: &str,
    field: &str,
    can_remove_call: bool,
) -> Option<Diagnostic> {
    let members =
        account_members::resolved_struct_members(document, workspace_index, receiver_type)?;
    if !members.members.iter().any(|member| member.name == field) {
        return None;
    }
    let mut data = serde_json::json!({
        "topic": TOPIC,
        "reason": FIELD_CALLED_AS_METHOD_REASON,
        "receiver": receiver_path,
        "receiverType": receiver_type,
        "field": field,
        "evidenceSource": EVIDENCE_SOURCE,
        "confidence": Confidence::Derived.as_str(),
        "applicability": Applicability::Unspecified.as_str(),
    });
    if can_remove_call {
        data["quickfix"] = serde_json::json!("remove-handler-field-call");
    }

    Some(diagnostic_from_range(
        range,
        AnchorDiagnosticKind::AnchorMissingAccountReference,
        format!(
            "`{receiver_path}.{field}()` calls `{field}` as a method, but `{receiver_type}` exposes `{field}` as a field; use `{receiver_path}.{field}`."
        ),
        Some(data),
    ))
}

fn unknown_method_diagnostic(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    range: Range,
    receiver_path: &str,
    receiver_type: &str,
    method_name: &str,
) -> Option<Diagnostic> {
    let members = account_members::resolved_struct_chain_completion_members(
        document,
        workspace_index,
        receiver_type,
        &[],
    )?;
    let candidates = method_candidates(&members);
    if candidates.iter().any(|candidate| candidate == method_name) {
        return None;
    }

    Some(diagnostic_from_range(
        range,
        AnchorDiagnosticKind::AnchorMissingAccountReference,
        format!(
            "`{receiver_path}.{method_name}()` does not resolve; `{}` has no method `{method_name}`.",
            members.owner_type
        ),
        Some(serde_json::json!({
            "topic": TOPIC,
            "reason": UNKNOWN_METHOD_REASON,
            "receiver": receiver_path,
            "receiverType": receiver_type,
            "method": method_name,
            "ownerType": members.owner_type,
            "candidates": candidates,
            "evidenceSource": EVIDENCE_SOURCE,
            "confidence": Confidence::Derived.as_str(),
            "applicability": Applicability::Unspecified.as_str(),
        })),
    ))
}

fn method_candidates(members: &account_members::ResolvedAccountMembers) -> Vec<String> {
    let mut seen = HashSet::new();
    members
        .members
        .iter()
        .filter(|member| member.completion_kind == CompletionItemKind::METHOD)
        .filter_map(|member| method_name_from_completion(&member.name))
        .filter(|name| seen.insert(name.clone()))
        .collect()
}

fn method_name_from_completion(label: &str) -> Option<String> {
    label
        .split_once('(')
        .map(|(name, _)| name)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}
