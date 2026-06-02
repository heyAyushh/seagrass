use {
    crate::{
        diagnostics::{
            bind_current_document_related_uri, collect, diagnostic_from_range,
            enrich_confidence_related_information, enrich_current_document_related_information,
            registry::AnchorDiagnosticKind, CURRENT_DOCUMENT_PLACEHOLDER_URI,
        },
        document::ParsedDocument,
    },
    tower_lsp::lsp_types::{
        Diagnostic, DiagnosticRelatedInformation, NumberOrString, Position, Range, Url,
    },
};

#[test]
fn bind_current_document_related_uri_rewrites_placeholder_only() {
    let target_uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let external_uri = Url::parse("file:///workspace/Cargo.toml").unwrap();
    let placeholder_uri = Url::parse(CURRENT_DOCUMENT_PLACEHOLDER_URI).unwrap();

    let mut diagnostics = vec![Diagnostic {
        range: Range::default(),
        severity: None,
        code: None,
        code_description: None,
        source: None,
        message: "msg".to_string(),
        related_information: Some(vec![
            DiagnosticRelatedInformation {
                location: tower_lsp::lsp_types::Location {
                    uri: placeholder_uri,
                    range: Range::default(),
                },
                message: "current document".to_string(),
            },
            DiagnosticRelatedInformation {
                location: tower_lsp::lsp_types::Location {
                    uri: external_uri.clone(),
                    range: Range::default(),
                },
                message: "external".to_string(),
            },
        ]),
        tags: None,
        data: None,
    }];

    bind_current_document_related_uri(&mut diagnostics, &target_uri);

    let related = diagnostics[0].related_information.as_ref().unwrap();
    assert_eq!(related[0].location.uri, target_uri);
    assert_eq!(related[1].location.uri, external_uri);
}

#[test]
fn generated_parser_rules_classify_anchor_constraint_shape_diagnostics() {
    let source = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut, mut)]
    pub state: Account<'info, State>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let diagnostic = collect(&document)
        .into_iter()
        .find(|diagnostic| diagnostic.message.contains("`mut` is duplicated"))
        .unwrap();

    assert!(!diagnostic.message.contains("mut already provided"));
    assert!(matches!(
        diagnostic.code.as_ref(),
        Some(NumberOrString::String(code)) if code == "anchor-constraint-shape"
    ));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("constraint"))
            .and_then(|value| value.as_str()),
        Some("mut")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("parserRule"))
            .and_then(|rule| rule.get("kind"))
            .and_then(|value| value.as_str()),
        Some("duplicate")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("parserRule"))
            .and_then(|rule| rule.get("message"))
            .and_then(|value| value.as_str()),
        Some("mut already provided")
    );
    let related = diagnostic
        .related_information
        .as_ref()
        .expect("constraint diagnostics should point at source evidence");
    assert!(
        related
            .iter()
            .any(|info| info.message == "`mut` constraint declaration."),
        "expected related information for the duplicated `mut` constraint; got {related:?}"
    );
}

#[test]
fn semantic_related_information_enriches_structured_diagnostic_data() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let mut diagnostics = vec![diagnostic_from_range(
        Range {
            start: Position {
                line: 3,
                character: 15,
            },
            end: Position {
                line: 3,
                character: 19,
            },
        },
        AnchorDiagnosticKind::AnchorConstraintShape,
        "synthetic structured diagnostic".to_string(),
        Some(serde_json::json!({
            "account": "state",
            "accountsStruct": "Create",
            "constraint": "init",
        })),
    )];

    enrich_current_document_related_information(&document, &mut diagnostics);

    let related = diagnostics[0]
        .related_information
        .as_ref()
        .expect("structured diagnostic should be enriched");
    for expected in [
        "`init` constraint declaration.",
        "Account field declaration for `state`.",
        "Accounts context declaration for `Create`.",
    ] {
        assert!(
            related.iter().any(|info| info.message.contains(expected)),
            "missing related information containing {expected:?}; got {related:?}"
        );
    }
}

#[test]
fn confidence_related_information_is_prepended_for_editor_peek() {
    let mut diagnostics = vec![Diagnostic {
        range: Range::default(),
        severity: None,
        code: None,
        code_description: None,
        source: None,
        message: "synthetic diagnostic".to_string(),
        related_information: None,
        tags: None,
        data: Some(serde_json::json!({
            "confidence": "heuristic",
            "topic": "seagrass/security.owner-check",
        })),
    }];

    enrich_confidence_related_information(&mut diagnostics);

    let related = diagnostics[0]
        .related_information
        .as_ref()
        .expect("confidence metadata");
    assert!(related[0]
        .message
        .contains("Seagrass confidence: heuristic; topic: seagrass/security.owner-check"));
}
