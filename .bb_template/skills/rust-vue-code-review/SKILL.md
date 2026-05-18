---
name: rust-vue-code-review
description: Review Blackboard Rust backend and Vue frontend changes from a structured ResourceBundle without editing files.
---

# Rust + Vue Code Review Skill

Use this skill when a TaskGraph node receives a
`task_graph_code_review_resource_bundle`.

## Operating Contract

- Treat `resource_bundle` as the authority for the review scope.
- Use `resource_bundle.data_sources.source_root` as the source/document root.
- Use `resource_bundle.data_sources.git.diff_patch`, `diff_stat`,
  `changed_files`, and `untracked_samples` as the primary change context.
- Use `resource_bundle.data_sources.static_checks` as deterministic evidence.
- Do not write, edit, delete, format, or fix files. This is review-only.
- The final Markdown report is written by `system_write_output`, not by you.

## Review Standard

Findings are the product. Put them first and order them by severity:
`critical`, `high`, `medium`, `low`, `nit`.

Each finding must include:

- Severity.
- File path and line number when available.
- The concrete bug, regression, safety risk, or missing test.
- Why it matters.
- A specific fix direction.
- Evidence from diff, static check output, or source anchors.

If there are no findings, say that explicitly and still list residual risks or
testing gaps.

## Rust Checklist

- Static checks: respect `cargo fmt --check`, `cargo clippy --all-targets`, and
  compiler output before making subjective comments.
- Correctness: ownership/lifetime mistakes, accidental clones, panic paths,
  unchecked `unwrap`/`expect`, path traversal, file IO atomicity, async blocking,
  cancellation, timeout, retry, and concurrency races.
- API quality: naming, trait bounds, error types, `Send`/`Sync`, `From`/`TryFrom`,
  `AsRef`, `Default`, `Debug`, serialization contracts, and future compatibility.
- Tests: changed behavior should have unit/integration coverage, especially for
  filesystem, task graph runtime, queueing, and Windows path behavior.
- Security: shell command construction, path escaping, environment injection,
  credential exposure, and user-controlled output paths.

## Vue Checklist

- Static checks: respect `vue-tsc` / `npm run build` output and ESLint output
  when present.
- Vue correctness: prop/emit typing, reactive state ownership, computed/watch
  side effects, stale refs, route updates, async loading states, and component
  lifecycle cleanup.
- UX correctness: loading/error/empty states, accessibility labels, keyboard
  behavior, responsive layout, i18n, and theme consistency.
- Security: unsafe `v-html`, unescaped markdown/html, URL handling, and user
  generated content.
- Maintainability: component boundaries, composables, duplicated state, overly
  broad watchers, and inconsistent design-system usage.

## Output Format

Return Chinese Markdown only.

Use this structure:

```markdown
# Code Review Report

## Findings
- [severity] path:line - title
  Impact: ...
  Evidence: ...
  Fix: ...

## Static Checks
- check_name: pass/fail/skipped, summary

## Coverage
- Reviewed scope: ...
- Not reviewed / low confidence: ...

## Verdict
pass | needs_changes | blocked

## Residual Risks
- ...
```

Do not include generic advice unless it is tied to this change.
