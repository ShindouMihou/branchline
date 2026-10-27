# Product backlog

## Stash workspace

Queued at the user's request on 2026-10-27. This is a main Git workspace feature, separate from structured stash previews in AI approvals.

- Save all working changes, selected files, or only unstaged changes while keeping staged changes.
- Choose whether to include untracked files; make ignored-file inclusion an explicit choice.
- Name a stash and preview exactly which changes are being saved.
- Browse stashes with date, branch, message, changed files, and diffs.
- Apply or pop a stash with a clear explanation of the difference, and an option to restore the index where supported.
- Rename and delete individual stashes; confirm destructive deletion.
- Explain conflicts and preserve a recovery path. Never report a restore as successful solely because an earlier rebase succeeded.
- Refresh the stash list automatically after local or approved AI operations.

Acceptance: ordinary UI actions and AI approval previews use consistent terms, show file scope explicitly, and report the actual operation result. Stashing only unstaged changes must preserve both the user's staged selection and their working-file contents, including partially staged files.
