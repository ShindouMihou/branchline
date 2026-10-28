# Product backlog

## Optional AI auto-approval

Implemented locally at the user's request on 2026-10-28. See [approval settings and policy behavior](AI_AUTO_APPROVAL.md). Manual approval remains the default.

- [x] Native opt-in settings for selected operations or all requests during the current broker session.
- [x] Connection/repository scope, visible expiry, pause and revocation across windows.
- [x] Conservative whole-command authorization classification, separate from explanatory previews.
- [x] Shared broker authorization with existing stale-preview checks and exactly-once execution.
- [x] Policy-attributed activity receipts, MCP guidance and regression coverage.

## Stash workspace

Started at the user's request on 2026-10-27; core stash management implemented locally on 2026-10-28. This is a main Git workspace feature, separate from structured stash previews in AI approvals.

- [x] Save all working changes, selected files, or only unstaged changes while keeping staged changes.
- [x] Opt in to untracked files.
- [ ] Explicit ignored-file inclusion (currently always excluded).
- [x] Name a stash and preview exactly which changes are being saved.
- [x] Browse stashes with date, branch, message, changed files, and diffs.
- [x] Apply or pop a stash with a clear explanation of the difference, and an option to restore the index where supported.
- [x] Delete individual stashes with confirmation and a recovery reference.
- [ ] Rename individual stashes.
- [x] Explain conflicts and preserve a recovery path. Never report a restore as successful solely because an earlier rebase succeeded.
- [x] Refresh the stash list automatically after local or approved AI operations.

Acceptance: ordinary UI actions and AI approval previews use consistent terms, show file scope explicitly, and report the actual operation result. Stashing only unstaged changes must preserve both the user's staged selection and their working-file contents, including partially staged files.
