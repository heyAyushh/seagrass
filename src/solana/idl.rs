use serde_json::Value;

pub(crate) fn program_name(value: &Value) -> Option<String> {
    value
        .get("program")
        .and_then(|program| program.get("name"))
        .or_else(|| {
            value
                .get("metadata")
                .and_then(|metadata| metadata.get("name"))
        })
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(crate) fn address(value: &Value) -> Option<String> {
    value
        .get("program")
        .and_then(|program| program.get("publicKey"))
        .or_else(|| value.get("address"))
        .or_else(|| value.get("programId"))
        .or_else(|| {
            value
                .get("metadata")
                .and_then(|metadata| metadata.get("address"))
        })
        .or_else(|| {
            value
                .get("metadata")
                .and_then(|metadata| metadata.get("programId"))
        })
        .and_then(Value::as_str)
        .map(str::to_string)
}
