---
name: completion-finisher
description: Require docs-freshness checks and grouped well-documented commit output at task completion.
user-invocable: true
---

# Skill: completion-finisher

## Purpose

Standardize task completion for coding work.
This skill acts as a completion checkpoint and does not replace writer docs freshness ownership.

## Inputs

- Changed files list.
- User-facing behavior and protocol changes.
- Test and docs update status.

## Checks

1. Writer docs freshness output exists after coding changes.
2. Required README/spec/changelog updates are complete.
3. Commit groups are logically scoped and non-overlapping.
4. Each commit message is clear and audit-friendly.

## Output

- Writer docs freshness pass/fail.
- Required doc updates.
- Grouped commit plan and message suggestions.
