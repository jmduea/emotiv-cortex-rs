---
name: docs-freshness
description: Detect doc drift and require README/spec/changelog updates when code or protocol changes.
user-invocable: true
---

# Skill: docs-freshness

## Purpose

Map source/protocol changes to required documentation updates.
This skill is executed under writer-owned docs freshness flow.

## Inputs

- Changed files in Rust/Python/docs trees.
- Public API/behavior/protocol deltas.

## Checks

1. Public behavior changed -> `CHANGELOG.md` reviewed.
2. Protocol changed -> protocol docs and examples updated.
3. New config/flags added -> README/spec/docs updated.
4. Notebook/tutorial paths still valid.

## Output

- Required doc updates list.
- Blocking doc gaps.
- Docs freshness verdict (pass/fail).
