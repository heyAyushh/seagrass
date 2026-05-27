use {
    super::{paths, push_unique_symbol, BridgeSymbol},
    std::{collections::HashSet, fs, path::Path},
    tower_lsp::lsp_types::{Location, SymbolKind, Url},
};

const MAX_IDL_FILES_PER_ROOT: usize = 64;

pub(super) fn collect_idl_symbols(
    root: &Path,
    symbols: &mut Vec<BridgeSymbol>,
    seen: &mut HashSet<String>,
) {
    for path in paths::idl_paths(root)
        .into_iter()
        .take(MAX_IDL_FILES_PER_ROOT)
    {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let Ok(uri) = Url::from_file_path(&path) else {
            continue;
        };
        collect_named_idl_array(
            &json,
            &text,
            &uri,
            "instructions",
            SymbolKind::FUNCTION,
            symbols,
            seen,
        );
        collect_named_idl_array(
            &json,
            &text,
            &uri,
            "accounts",
            SymbolKind::STRUCT,
            symbols,
            seen,
        );
        collect_named_idl_array(
            &json,
            &text,
            &uri,
            "types",
            SymbolKind::STRUCT,
            symbols,
            seen,
        );
        collect_named_idl_array(
            &json,
            &text,
            &uri,
            "events",
            SymbolKind::EVENT,
            symbols,
            seen,
        );
        collect_named_idl_array(
            &json,
            &text,
            &uri,
            "errors",
            SymbolKind::ENUM_MEMBER,
            symbols,
            seen,
        );
        if let Some(program) = json.get("program") {
            collect_named_idl_array(
                program,
                &text,
                &uri,
                "instructions",
                SymbolKind::FUNCTION,
                symbols,
                seen,
            );
            collect_named_idl_array(
                program,
                &text,
                &uri,
                "accounts",
                SymbolKind::STRUCT,
                symbols,
                seen,
            );
            collect_named_idl_array(
                program,
                &text,
                &uri,
                "definedTypes",
                SymbolKind::STRUCT,
                symbols,
                seen,
            );
            collect_named_idl_array(
                program,
                &text,
                &uri,
                "events",
                SymbolKind::EVENT,
                symbols,
                seen,
            );
            collect_named_idl_array(
                program,
                &text,
                &uri,
                "errors",
                SymbolKind::ENUM_MEMBER,
                symbols,
                seen,
            );
        }
    }
}

fn collect_named_idl_array(
    json: &serde_json::Value,
    text: &str,
    uri: &Url,
    key: &'static str,
    kind: SymbolKind,
    symbols: &mut Vec<BridgeSymbol>,
    seen: &mut HashSet<String>,
) {
    let Some(items) = json.get(key).and_then(|value| value.as_array()) else {
        return;
    };
    for item in items {
        let Some(name) = item.get("name").and_then(|value| value.as_str()) else {
            continue;
        };
        let container_name = Some(format!("IDL {key}"));
        push_unique_symbol(
            symbols,
            seen,
            BridgeSymbol {
                name: name.to_string(),
                kind,
                location: Location {
                    uri: uri.clone(),
                    range: json_name_range(text, name).unwrap_or_default(),
                },
                container_name: container_name.clone(),
                type_display: None,
            },
        );
        collect_idl_child_symbols(item, text, uri, key, name, symbols, seen);
    }
}

fn collect_idl_child_symbols(
    item: &serde_json::Value,
    text: &str,
    uri: &Url,
    key: &str,
    parent_name: &str,
    symbols: &mut Vec<BridgeSymbol>,
    seen: &mut HashSet<String>,
) {
    if key == "instructions" {
        collect_idl_fields(
            item.get("args").and_then(|value| value.as_array()),
            text,
            uri,
            SymbolKind::VARIABLE,
            format!("IDL instruction {parent_name}"),
            symbols,
            seen,
        );
        return;
    }

    let fields = item
        .get("type")
        .and_then(|value| value.get("fields"))
        .and_then(|value| value.as_array())
        .or_else(|| item.get("fields").and_then(|value| value.as_array()));
    collect_idl_fields(
        fields,
        text,
        uri,
        SymbolKind::FIELD,
        parent_name.to_string(),
        symbols,
        seen,
    );
}

fn collect_idl_fields(
    fields: Option<&Vec<serde_json::Value>>,
    text: &str,
    uri: &Url,
    kind: SymbolKind,
    container_name: String,
    symbols: &mut Vec<BridgeSymbol>,
    seen: &mut HashSet<String>,
) {
    let Some(fields) = fields else {
        return;
    };
    for field in fields {
        let Some(name) = field.get("name").and_then(|value| value.as_str()) else {
            continue;
        };
        push_unique_symbol(
            symbols,
            seen,
            BridgeSymbol {
                name: name.to_string(),
                kind,
                location: Location {
                    uri: uri.clone(),
                    range: json_name_range(text, name).unwrap_or_default(),
                },
                container_name: Some(container_name.clone()),
                type_display: field.get("type").map(idl_type_display),
            },
        );
    }
}

fn json_name_range(text: &str, name: &str) -> Option<tower_lsp::lsp_types::Range> {
    paths::range_for_json_value(text, name)
}

fn idl_type_display(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Object(object) => {
            if let Some(defined) = object.get("defined") {
                return idl_defined_type_display(defined);
            }
            if let Some(option) = object.get("option") {
                return format!("Option<{}>", idl_type_display(option));
            }
            if let Some(vec) = object.get("vec") {
                return format!("Vec<{}>", idl_type_display(vec));
            }
            if let Some(array) = object.get("array").and_then(|value| value.as_array()) {
                if let Some(inner) = array.first() {
                    let len = array.get(1).map_or("?".to_string(), json_scalar_display);
                    return format!("[{}; {len}]", idl_type_display(inner));
                }
            }
            value.to_string()
        }
        _ => value.to_string(),
    }
}

fn idl_defined_type_display(value: &serde_json::Value) -> String {
    if let Some(name) = value.as_str() {
        return name.to_string();
    }
    value
        .get("name")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn json_scalar_display(value: &serde_json::Value) -> String {
    value
        .as_u64()
        .map(|value| value.to_string())
        .or_else(|| value.as_str().map(str::to_string))
        .unwrap_or_else(|| value.to_string())
}
