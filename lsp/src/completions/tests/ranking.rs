use super::*;

#[test]
fn ranks_missing_init_companion_constraints_above_generic_snippets() {
    let source = "#[account(init, )]\npub state: Account<'info, State>,";
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(
        &document,
        Position {
            line: 0,
            character: "#[account(init, ".len() as u32,
        },
    )
    .expect("expected account constraint completions after init");

    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("payer =")
    );
    assert_eq!(
        items
            .first()
            .and_then(|item| item.data.as_ref())
            .and_then(|data| data.get("pendingFixTarget"))
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert!(items
        .first()
        .and_then(|item| item.sort_text.as_deref())
        .is_some_and(|sort_text| sort_text.starts_with("000_seagrass_rank_pending_fix")));
}

#[test]
fn global_ranking_context_orders_pending_fix_and_type_matches_across_items() {
    let source = "#[account(init, )]\npub state: Account<'info, State>,";
    let document = ParsedDocument::parse_or_empty(source);
    let cursor = CursorContext::classify_document(
        &document,
        Position {
            line: 0,
            character: "#[account(init, ".len() as u32,
        },
    );
    let ranking_context = crate::completions::ranking::RankingContext::new(&cursor);
    let mut items = vec![
        CompletionItem {
            label: "fallback".to_string(),
            sort_text: Some("000_local_bucket_fallback".to_string()),
            ..CompletionItem::default()
        },
        CompletionItem {
            label: "matched".to_string(),
            data: Some(serde_json::json!({ "typeMatched": true })),
            sort_text: Some("999_local_bucket_matched".to_string()),
            ..CompletionItem::default()
        },
        CompletionItem {
            label: "payer =".to_string(),
            data: Some(serde_json::json!({ "pendingFixTarget": true })),
            sort_text: Some("999_local_bucket_pending".to_string()),
            ..CompletionItem::default()
        },
    ];

    crate::completions::ranking::rank_completion_items(&ranking_context, &mut items);
    items.sort_by(|left, right| {
        left.sort_text
            .cmp(&right.sort_text)
            .then_with(|| left.label.cmp(&right.label))
    });

    assert_eq!(items[0].label, "payer =");
    assert_eq!(items[1].label, "matched");
    assert!(items[0]
        .sort_text
        .as_deref()
        .is_some_and(|sort_text| sort_text.starts_with("000_seagrass_rank_pending_fix")));
    assert!(items[1]
        .sort_text
        .as_deref()
        .is_some_and(|sort_text| sort_text.starts_with("010_seagrass_rank_type_match")));
}
