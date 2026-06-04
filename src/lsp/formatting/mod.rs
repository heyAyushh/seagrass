use {
    std::{
        fs,
        io::Write,
        path::PathBuf,
        process::{Command, Stdio},
        time::{Duration, SystemTime, UNIX_EPOCH},
    },
    tower_lsp::lsp_types::{Position, Range, TextEdit},
    wait_timeout::ChildExt,
};

const RUSTFMT_COMMAND: &str = "rustfmt";
const RUSTFMT_EDITION: &str = "2021";
const RUSTFMT_TIMEOUT_SECS: u64 = 3;
const RUSTFMT_TEMP_PREFIX: &str = "seagrass-rustfmt";

pub(crate) fn format_document(source: &str) -> Option<Vec<TextEdit>> {
    let formatted = rustfmt_source(source)?;
    if formatted == source {
        return Some(Vec::new());
    }

    Some(vec![TextEdit {
        range: full_document_range(source),
        new_text: formatted,
    }])
}

fn rustfmt_source(source: &str) -> Option<String> {
    let output_path = rustfmt_output_path();
    let output_file = fs::File::create(&output_path).ok()?;
    let mut child = match Command::new(RUSTFMT_COMMAND)
        .args(["--edition", RUSTFMT_EDITION, "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::from(output_file))
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            let _ = fs::remove_file(&output_path);
            return None;
        }
    };

    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_file(&output_path);
        return None;
    };
    if stdin.write_all(source.as_bytes()).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_file(&output_path);
        return None;
    }
    drop(stdin);

    let status = match child
        .wait_timeout(Duration::from_secs(RUSTFMT_TIMEOUT_SECS))
        .ok()?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&output_path);
            return None;
        }
    };
    if !status.success() {
        let _ = fs::remove_file(&output_path);
        return None;
    }
    let formatted = fs::read_to_string(&output_path).ok();
    let _ = fs::remove_file(&output_path);
    formatted
}

fn rustfmt_output_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "{RUSTFMT_TEMP_PREFIX}-{}-{nanos}.rs",
        std::process::id()
    ))
}

fn full_document_range(source: &str) -> Range {
    let mut line = 0u32;
    let mut character = 0u32;

    for segment in source.split_inclusive('\n') {
        if segment.ends_with('\n') {
            line = line.saturating_add(1);
            character = 0;
        } else {
            character = u32::try_from(segment.chars().count()).unwrap_or(u32::MAX);
        }
    }

    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position { line, character },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_whole_rust_document() {
        let edits = format_document("fn main(){println!(\"hi\");}\n").expect("rustfmt output");

        assert_eq!(edits.len(), 1);
        assert_eq!(
            edits[0].range,
            full_document_range("fn main(){println!(\"hi\");}\n")
        );
        assert_eq!(edits[0].new_text, "fn main() {\n    println!(\"hi\");\n}\n");
    }

    #[test]
    fn returns_empty_edits_when_document_is_already_formatted() {
        let edits =
            format_document("fn main() {\n    println!(\"hi\");\n}\n").expect("rustfmt output");

        assert!(edits.is_empty());
    }

    #[test]
    fn returns_none_when_rustfmt_cannot_parse_source() {
        assert!(format_document("fn main( {\n").is_none());
    }

    #[test]
    fn full_range_handles_trailing_newline() {
        let range = full_document_range("fn main() {}\n");

        assert_eq!(range.end.line, 1);
        assert_eq!(range.end.character, 0);
    }
}
