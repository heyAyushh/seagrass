# Release Readiness Evidence

Status: Active

Two release proofs cannot be produced by local tests:

- clean fuzz workflow covering 24 aggregate fuzz-hours
- external review signoff, either paid review or the 30-day community audit path

The release workflow enforces those proofs with:

```sh
bun scripts/check-release-readiness.ts --version "$(tr -d '[:space:]' < VERSION)"
```

Before tagging locally, run the full production preflight in release mode:

```sh
bun scripts/verify-production.ts --release
```

Release mode runs the normal production gate but switches release-readiness
validation from pending-shape checks to strict evidence checks for the current
commit.

Use `--readiness-path <path>` to validate a generated evidence artifact before
copying it into `docs/release-readiness.json`.

The checked-in `docs/release-readiness.json` is intentionally pending until
real evidence exists. Pending evidence uses explicit `pending` sentinels and
zero fuzz hours so it cannot be mistaken for proof. For local shape validation
while the evidence is pending:

```sh
bun scripts/check-release-readiness.ts \
  --allow-pending \
  --version "$(tr -d '[:space:]' < VERSION)"
```

## Evidence Fields

| field | requirement |
| --- | --- |
| `releaseVersion` | Must match the release tag version. |
| `fuzzCleanRun.status` | Must be `passed`. |
| `fuzzCleanRun.workflowRunUrl` | concrete GitHub Actions run URL under this repo with a positive run id. |
| `fuzzCleanRun.commit` | Must match the release commit. |
| `fuzzCleanRun.startedAt` / `completedAt` | Must be valid workflow timestamps with `completedAt` after `startedAt`. |
| `fuzzCleanRun.aggregateFuzzHours` | Must be at least 24 aggregate fuzz-hours across the sharded targets. |
| `fuzzCleanRun.corpusSha256` | Required after `status` is `passed`; SHA-256 tree hash of `fuzz/corpus` after importing the workflow artifacts. |
| `fuzzCleanRun.targets` | Must list the fuzz targets covered. |
| `fuzzCleanRun.workflowMatrixTargets` | Must match `.github/workflows/fuzz.yaml` and `fuzz/Cargo.toml`. |
| `fuzzCleanRun.targetRuns` | Required per-target proof rows; `shards` must match `.github/workflows/fuzz.yaml`, and hours must sum to `aggregateFuzzHours`. |
| `externalReview.status` | Must be `signed-off`. |
| `externalReview.mode` | `paid` or `community`. |
| `externalReview.reviewer` | Reviewer, firm, or audit-period owner. |
| `externalReview.artifactUrl` | Concrete pull, issue, or discussion URL under this repo. |
| `externalReview.auditOpenedAt` | Required for `community`; start timestamp for the public review window. |
| `externalReview.announcementUrl` | Required for `community`; concrete pull, issue, or discussion URL under this repo. |
| `externalReview.auditWindowDays` | Computed from `auditOpenedAt` to `completedAt`; must be at least 30 for `community`. |
| `externalReview.signoffs` | Required after `status` is `signed-off`; at least one reviewer signoff with a concrete pull, issue, or discussion URL. |
| `externalReview.findingsDisposition` | Required after `status` is `signed-off`; must include an immutable CHANGELOG.md proof URL under this repo, using either the release tag (`v...`) or a 40-character commit SHA. |

## Updating The Evidence

After the fuzz workflow has produced the required clean run, update
`fuzzCleanRun` with the workflow URL, release commit, timestamps, target list,
and aggregate hours.

The long fuzz workflow uploads `fuzz-readiness-<sha>.json` and shard corpus
archives. Download them with `gh`, then import the corpus and readiness object:

```sh
gh run download <run-id> --dir target/fuzz-artifacts
bun scripts/import-fuzz-artifacts.ts \
  --artifacts-dir target/fuzz-artifacts \
  --commit <release-sha> \
  --dry-run
bun scripts/import-fuzz-artifacts.ts \
  --artifacts-dir target/fuzz-artifacts \
  --commit <release-sha>
```

The importer rejects stale target matrices, stale shard counts, missing shard
archives, unsafe tar members, placeholder workflow run ids, insufficient
aggregate fuzz-hour evidence, and mismatched commits. It also writes `fuzzCleanRun.corpusSha256`,
which the strict readiness gate compares against the checked-in
`fuzz/corpus` tree before release packaging can proceed.
The release-readiness regression suite rejects placeholder workflow run ids,
rejects generic review proof URLs, rejects mutable changelog proof URLs, and
rejects stale corpus hashes.
The release workflow packages the checked `release-readiness.json` with the
promoted fuzz corpus so release users can replay the corpus against the exact
proof that unlocked the tag.

After external review signs off, update `externalReview` with the review mode,
reviewer, artifact URL, completion timestamp, and community audit window if
using the community path.

Generate the object from the signoff and changelog disposition artifacts:

```sh
bun scripts/review-readiness.ts \
  --mode community \
  --reviewer <reviewer-or-firm> \
  --artifact-url <review-signoff-url> \
  --findings-artifact-url <changelog-proof-url> \
  --audit-opened-at <community-audit-opened-at> \
  --announcement-url <community-audit-announcement-url> \
  --out target/review-readiness.json
```

Do not change `status` to `passed` or `signed-off` without an artifact URL that
can be inspected later. Generic repo pages such as `/pulls`, placeholder ids
such as `actions/runs/0`, and `issues/0` are rejected; final evidence must point
at concrete review and workflow artifacts. Changelog disposition proof must use
an immutable `blob/<v... or sha>/CHANGELOG.md` URL, not `main` or `master`.

When both generated objects exist, apply them with the strict merge command. It
validates the merged file before writing `docs/release-readiness.json`:

```sh
bun scripts/apply-release-readiness.ts \
  --fuzz target/fuzz-readiness-<release-sha>.json \
  --review target/review-readiness.json \
  --version "$(tr -d '[:space:]' < VERSION)" \
  --commit <release-sha>
```

The release workflow tests additionally reject mutable VSIX release metadata URLs
and unpinned workflow actions so packaged editor metadata and GitHub Actions
dependencies stay tied to immutable release inputs.
