use {
    crate::{actions, server_types::ServerSettings},
    serde_json::{Map, Value},
    std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        task::{Context, Poll},
    },
    tower::Service,
    tower_lsp::{
        jsonrpc::{Request, Response},
        lsp_types::{ClientCapabilities, Range},
    },
};

const CODE_ACTION_METHOD: &str = "textDocument/codeAction";
const CODE_ACTION_RESOLVE_METHOD: &str = "codeAction/resolve";
const SNIPPET_TEXT_EDIT_CAPABILITY: &str = "snippetTextEdit";
const SNIPPET_INSERT_TEXT_FORMAT: u8 = 2;

pub(super) struct SnippetEditService<S> {
    inner: S,
    client_supports_snippet_edits: Arc<AtomicBool>,
    settings: Arc<Mutex<ServerSettings>>,
}

impl<S> SnippetEditService<S> {
    pub(super) fn new(
        inner: S,
        client_supports_snippet_edits: Arc<AtomicBool>,
        settings: Arc<Mutex<ServerSettings>>,
    ) -> Self {
        Self {
            inner,
            client_supports_snippet_edits,
            settings,
        }
    }

    fn should_emit_snippets(&self) -> bool {
        self.client_supports_snippet_edits.load(Ordering::Relaxed)
            && !self
                .settings
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .agent_mode
    }
}

impl<S> Service<Request> for SnippetEditService<S>
where
    S: Service<Request, Response = Option<Response>> + Send + 'static,
    S::Error: Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Option<Response>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let should_rewrite_response = matches!(
            request.method(),
            CODE_ACTION_METHOD | CODE_ACTION_RESOLVE_METHOD
        ) && self.should_emit_snippets();
        let response = self.inner.call(request);
        Box::pin(async move {
            let response = response.await?;
            if should_rewrite_response {
                Ok(response.map(rewrite_code_action_response))
            } else {
                Ok(response)
            }
        })
    }
}

pub(super) fn client_supports_snippet_text_edit(capabilities: &ClientCapabilities) -> bool {
    capabilities
        .experimental
        .as_ref()
        .and_then(|experimental| experimental.get(SNIPPET_TEXT_EDIT_CAPABILITY))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn rewrite_code_action_response(response: Response) -> Response {
    let (id, body) = response.into_parts();
    Response::from_parts(id, body.map(rewrite_code_action_result))
}

pub(super) fn rewrite_code_action_result(mut result: Value) -> Value {
    let Some(actions) = result.as_array_mut() else {
        return result;
    };
    let mut snippet_document_edit_used = false;
    for action in actions {
        rewrite_action(action, &mut snippet_document_edit_used);
    }
    result
}

fn rewrite_action(action: &mut Value, snippet_document_edit_used: &mut bool) {
    let Some(edit) = action.get_mut("edit") else {
        return;
    };
    rewrite_workspace_edit(edit, snippet_document_edit_used);
}

fn rewrite_workspace_edit(edit: &mut Value, snippet_document_edit_used: &mut bool) {
    let Some(edit_object) = edit.as_object_mut() else {
        return;
    };
    if edit_object.contains_key("documentChanges") {
        rewrite_document_changes(edit_object, snippet_document_edit_used);
        return;
    }
    let Some(changes) = edit_object.remove("changes") else {
        return;
    };
    if *snippet_document_edit_used || !changes_contain_snippet(&changes) {
        edit_object.insert("changes".to_string(), changes);
        return;
    }
    edit_object.insert(
        "documentChanges".to_string(),
        Value::Array(changes_to_document_changes(
            changes,
            snippet_document_edit_used,
        )),
    );
}

fn rewrite_document_changes(
    edit_object: &mut Map<String, Value>,
    snippet_document_edit_used: &mut bool,
) {
    let Some(document_changes) = edit_object
        .get_mut("documentChanges")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for document_change in document_changes {
        if *snippet_document_edit_used {
            return;
        }
        if rewrite_text_document_edit(document_change) {
            *snippet_document_edit_used = true;
        }
    }
}

fn changes_to_document_changes(
    changes: Value,
    snippet_document_edit_used: &mut bool,
) -> Vec<Value> {
    let Some(changes) = changes.as_object() else {
        return Vec::new();
    };
    let mut document_changes = Vec::with_capacity(changes.len());
    for (uri, edits) in changes {
        let mut document_change = serde_json::json!({
            "textDocument": {
                "uri": uri,
                "version": null
            },
            "edits": edits
        });
        if !*snippet_document_edit_used && rewrite_text_document_edit(&mut document_change) {
            *snippet_document_edit_used = true;
        }
        document_changes.push(document_change);
    }
    document_changes
}

fn rewrite_text_document_edit(document_change: &mut Value) -> bool {
    if document_change.get("textDocument").is_none() {
        return false;
    }
    let Some(edits) = document_change
        .get_mut("edits")
        .and_then(Value::as_array_mut)
    else {
        return false;
    };
    let mut rewrote_snippet = false;
    for edit in edits {
        if rewrite_text_edit(edit) {
            rewrote_snippet = true;
        }
    }
    rewrote_snippet
}

fn changes_contain_snippet(changes: &Value) -> bool {
    changes
        .as_object()
        .into_iter()
        .flat_map(Map::values)
        .filter_map(Value::as_array)
        .flatten()
        .any(text_edit_has_snippet_template)
}

fn rewrite_text_edit(edit: &mut Value) -> bool {
    let Some(template) = text_edit_snippet_template(edit) else {
        return false;
    };
    let Some(edit_object) = edit.as_object_mut() else {
        return false;
    };
    edit_object.insert("newText".to_string(), Value::String(template));
    edit_object.insert(
        "insertTextFormat".to_string(),
        Value::from(SNIPPET_INSERT_TEXT_FORMAT),
    );
    true
}

fn text_edit_has_snippet_template(edit: &Value) -> bool {
    text_edit_snippet_template(edit).is_some()
}

fn text_edit_snippet_template(edit: &Value) -> Option<String> {
    let range = serde_json::from_value::<Range>(edit.get("range")?.clone()).ok()?;
    let materialized = edit.get("newText")?.as_str()?;
    actions::snippet_template_for_edit(range, materialized)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        tower_lsp::lsp_types::{Position, Range},
    };

    fn sample_range() -> Range {
        Range {
            start: Position {
                line: 3,
                character: 4,
            },
            end: Position {
                line: 3,
                character: 4,
            },
        }
    }

    #[test]
    fn detects_snippet_text_edit_client_capability() {
        let capabilities = ClientCapabilities {
            experimental: Some(serde_json::json!({ "snippetTextEdit": true })),
            ..ClientCapabilities::default()
        };

        assert!(client_supports_snippet_text_edit(&capabilities));
    }

    #[test]
    fn rejects_missing_snippet_text_edit_client_capability() {
        let capabilities = ClientCapabilities {
            experimental: Some(serde_json::json!({ "snippetTextEdit": false })),
            ..ClientCapabilities::default()
        };

        assert!(!client_supports_snippet_text_edit(&capabilities));
        assert!(!client_supports_snippet_text_edit(
            &ClientCapabilities::default()
        ));
    }

    #[test]
    fn rewrites_changes_workspace_edit_to_snippet_document_changes() {
        let template = "    pub ${1:maker}: ${2:Signer<'info>},$0\n";
        let edit = actions::snippet_text_edit(sample_range(), template);
        let result = serde_json::json!([
            {
                "title": "Add `maker` to `Create`",
                "edit": {
                    "changes": {
                        "file:///tmp/lib.rs": [
                            {
                                "range": edit.range,
                                "newText": edit.new_text
                            }
                        ]
                    }
                }
            }
        ]);

        let rewritten = rewrite_code_action_result(result);
        let action = &rewritten.as_array().expect("actions")[0];
        let edit = action.get("edit").expect("workspace edit");
        assert!(edit.get("changes").is_none());
        let text_edit = &edit
            .get("documentChanges")
            .and_then(Value::as_array)
            .expect("document changes")[0]
            .get("edits")
            .and_then(Value::as_array)
            .expect("text edits")[0];

        assert_eq!(
            text_edit.get("newText").and_then(Value::as_str),
            Some(template)
        );
        assert_eq!(
            text_edit.get("insertTextFormat").and_then(Value::as_u64),
            Some(u64::from(SNIPPET_INSERT_TEXT_FORMAT))
        );
    }

    #[test]
    fn rewrites_only_one_text_document_edit_per_response() {
        let first = actions::snippet_text_edit(sample_range(), "${1:first}$0");
        let second = actions::snippet_text_edit(sample_range(), "${1:second}$0");
        let result = serde_json::json!([
            {
                "title": "First",
                "edit": {
                    "changes": {
                        "file:///tmp/first.rs": [
                            { "range": first.range, "newText": first.new_text }
                        ]
                    }
                }
            },
            {
                "title": "Second",
                "edit": {
                    "changes": {
                        "file:///tmp/second.rs": [
                            { "range": second.range, "newText": second.new_text }
                        ]
                    }
                }
            }
        ]);

        let rewritten = rewrite_code_action_result(result);
        let actions = rewritten.as_array().expect("actions");
        let first_text_edit = &actions[0]["edit"]["documentChanges"][0]["edits"][0];
        let second_edit = &actions[1]["edit"];

        assert_eq!(
            first_text_edit
                .get("insertTextFormat")
                .and_then(Value::as_u64),
            Some(u64::from(SNIPPET_INSERT_TEXT_FORMAT))
        );
        assert!(second_edit.get("changes").is_some());
    }
}
