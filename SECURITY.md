# Security Policy

## Scope

Security reports are in scope when they affect:

- malformed Rust, TOML, IDL, or artifact input handled by the LSP
- diagnostics, quick fixes, completions, hovers, or execute-command output that
  could mislead users into unsafe Solana behavior
- editor adapter behavior that executes unexpected commands
- CI, release artifacts, generated support catalogs, or fuzz targets under
  `src/`

Reports about upstream Anchor, Solana, Zed, VS Code, or Bun should also be sent
to the affected upstream project.

## Reporting

Preferred: use GitHub private vulnerability reporting for this repository.

Fallback: if private vulnerability reporting is unavailable, open a public issue
asking maintainers to enable a private reporting channel. Do not include exploit
details, crash inputs, credentials, or unreleased vulnerability details in the
public issue.

Please include:

- affected commit or release
- reproduction steps
- expected impact
- crash corpus or input file when relevant

## Response Timeline

- acknowledgement within 72 hours
- status update within 14 days
- coordinated disclosure target within 90 days, unless active exploitation or
  ecosystem risk requires faster publication

## Fuzzing

Malformed-input bugs are security-relevant. Attach reduced corpus files when a
fuzzer, property test, or parser recovery path finds a crash or panic.
