use {
    crate::{
        diagnostics::{diagnostic_from_range_with_related, registry::AnchorDiagnosticKind},
        document::ParsedDocument,
        project,
    },
    tower_lsp::lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Url},
};

pub const SYNC_DECLARE_ID_QUICKFIX: &str = "sync-declare-id";

pub fn collect(
    document: &ParsedDocument,
    uri: &Url,
    anchor_toml_uri: &Url,
    anchor_toml_text: &str,
) -> Vec<Diagnostic> {
    let Some(declared) = document.symbols().declared_program_id.as_ref() else {
        return Vec::new();
    };
    let Some(expected) = project::preferred_program_id(uri, anchor_toml_text) else {
        return Vec::new();
    };
    if expected.value == declared.value {
        return Vec::new();
    }

    vec![diagnostic_from_range_with_related(
        declared.range,
        AnchorDiagnosticKind::AnchorProjectId,
        format!(
            "`declare_id!` is `{}` but Anchor.toml declares `{}` for `{}` on `{}`.",
            declared.value, expected.value, expected.name, expected.cluster
        ),
        Some(serde_json::json!({
            "quickfix": SYNC_DECLARE_ID_QUICKFIX,
            "declaredProgramId": declared.value,
            "anchorTomlProgramId": expected.value,
            "program": expected.name,
            "cluster": expected.cluster,
            "anchorToml": anchor_toml_uri.as_str(),
        })),
        Some(vec![DiagnosticRelatedInformation {
            location: Location {
                uri: anchor_toml_uri.clone(),
                range: expected.range,
            },
            message: format!(
                "Anchor.toml declares `{}` for `{}` on `{}`.",
                expected.value, expected.name, expected.cluster
            ),
        }]),
    )]
}

#[cfg(test)]
mod tests {
    use {super::*, tower_lsp::lsp_types::NumberOrString};

    #[test]
    fn reports_declared_program_id_mismatch() {
        let document = ParsedDocument::parse(
            r#"
declare_id!("Declared111111111111111111111111111111111");
"#,
        )
        .unwrap();
        let uri = Url::parse("file:///tmp/demo/programs/my-program/src/lib.rs").unwrap();
        let anchor_toml_uri = Url::parse("file:///tmp/demo/Anchor.toml").unwrap();
        let diagnostics = collect(
            &document,
            &uri,
            &anchor_toml_uri,
            r#"
[programs.localnet]
my_program = "Expected111111111111111111111111111111111"
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(matches!(
            diagnostics[0].code.as_ref(),
            Some(NumberOrString::String(code)) if code == "anchor-project-id"
        ));
        assert!(diagnostics[0]
            .data
            .as_ref()
            .is_some_and(|data| data["quickfix"] == SYNC_DECLARE_ID_QUICKFIX));
        assert!(diagnostics[0].related_information.is_some());
    }

    #[test]
    fn keeps_matching_declared_program_id_clean() {
        let document = ParsedDocument::parse(
            r#"
declare_id!("Expected111111111111111111111111111111111");
"#,
        )
        .unwrap();
        let uri = Url::parse("file:///tmp/demo/programs/my-program/src/lib.rs").unwrap();
        let anchor_toml_uri = Url::parse("file:///tmp/demo/Anchor.toml").unwrap();
        let diagnostics = collect(
            &document,
            &uri,
            &anchor_toml_uri,
            r#"
[programs.localnet]
my_program = "Expected111111111111111111111111111111111"
"#,
        );

        assert!(diagnostics.is_empty());
    }
}
