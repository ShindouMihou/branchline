# Branchline 0.4

A Git client for macOS, built with Tauri 2, Rust, and TypeScript. The compact tool-window layout follows JetBrains IDEs and uses Git's explicit staging workflow.

## Run

Open a locally built `Branchline.app` from `src-tauri/target/release/bundle/macos/`. This Apple Silicon build targets macOS 12 or later. It is locally ad-hoc signed and is not notarized. Git must be installed and available on PATH (Apple's developer tools provide it).

Choose **Open repository**, then select an existing working repository. Command+O opens the folder picker; Command+R refreshes. Local file, index, branch, and history changes update automatically while the app is open. Checks pause during dialogs and interaction, preserve the selected diff, and never fetch or pull. The last repository and local commit drafts are remembered.

## Changes and commits

Select a changed file to inspect its side-by-side diff. **Unified** switches layouts. **Stage hunk** stages a text change; **+** beside a file stages the whole file. Select the file under **Staged** to review the commit contents. The **−** actions unstage without modifying working files.

Enter a message and choose **Commit** or press Command+Return. Only staged content is committed. Existing hooks and signing settings apply. Failures preserve the message and display Git's error.

## Branches and remotes

Click the branch name in the toolbar to search local and remote branches. Select a local branch to check it out. Select a remote branch to create a local tracking branch. **New branch** starts from HEAD; the log's **New branch from commit** starts from a selected commit. Git blocks switches that would overwrite local changes. There is no automatic stash.

**Update** fetches and pulls with a destination review; its dropdown provides **Fetch only**. **Push** remains a separate action:

- Fetch downloads branches and tags from a configured remote without pruning.
- Pull requires a clean working tree and performs a fast-forward only. Diverged history is left unchanged.
- Push explicitly sends HEAD to the chosen remote branch, establishes upstream tracking, and never forces an update.

The toolbar shows ahead/behind counts relative to upstream. Network operations use your configured credential helpers and SSH agent. The app does not collect credentials; if authentication requires a terminal prompt, authenticate with Git first.

## Git log and commit actions

The log shows a topological graph and a commit table with branch labels, author, date, and hash. Choose **All branches** or **Current branch**. Search filters the commits already loaded; **Load older commits** adds another 100 rows. Select a commit, then a changed file in the sidebar to review its historical diff. Merge diffs compare against the first parent.

Shift-click selects a range; Command-click toggles commits. Right-click or choose **Actions**:

- **Squash selected commits** combines consecutive commits, including ranges below HEAD, then replays later commits. Review the plan and edit the combined message.
- **Edit commit message** rewords one commit and replays later commits.
- **Cherry-pick** applies one selected commit as a new commit on the current branch.
- **Revert commit** creates a commit reversing the selected change.
- **New branch from commit** and **Copy commit hash** are also available.

Squash and reword require a clean working tree and a local branch. They reject nonconsecutive selections, merge commits in the affected range, and affected commits reachable from fetched remote-tracking refs. Fetch first: this check uses local refs, not live hosting-provider protection rules. Rewriting published history and force-pushing are not supported.

Before a rewrite, cherry-pick, or revert, the original HEAD is saved under `refs/branchline-backups/`. Successful history actions show a short confirmation; expand **Recovery details** for the backup ref. Inspect it with `git show <backup-ref>`. To recover without overwriting an existing branch, use `git switch -c recovered-work <backup-ref>`.

Failed rebase/cherry-pick/revert operations attempt to abort and restore their starting state. If Git remains in an operation, an **Abort** banner is available. Aborting may discard conflict-resolution edits; its dialog explains this before execution.

## Development

Prerequisites: Node.js 22+, npm, stable Rust, Xcode command-line tools, and Git.

```sh
npm ci
npm run desktop
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri -- build
```

The npm launcher uses your normal Rust installation. An optional developer-local toolchain can live in the ignored `.local/toolchain` directory.

## Implementation

- A framework-free TypeScript interface communicates with a serialized Rust service off the UI thread.
- Git receives argument arrays and literal pathspecs. Frontend operations use the repository root retained by the backend.
- Hunk staging rejects stale diffs. Commits reject stale staged-content fingerprints. Branch/network/commit actions check branch and HEAD snapshots. Rewrites revalidate the preview plan.
- Git owns index locks. Snapshot checks are optimistic; unrelated external Git processes are not locked out between validation and execution.
- Rewrites use Git interactive rebase with generated local editor scripts and files for the todo and message. Paths are shell-quoted. Auto-stash, auto-squash, and update-refs are disabled; other local branches retain their original commits.
- Read operations disable external diff and textconv drivers. Repository hooks and signing programs still execute through normal Git operations.
- Git operations have a 60-second timeout; the Git process group is terminated on timeout. Refresh and inspect state before retrying, especially after a timed-out push.
- The webview loads local assets. Remote requests occur through Git only when fetch/pull/push is explicitly run.
- Banners participate in normal document flow above the tool rail. Larger text, clearer secondary labels, taller log rows, and wrapped recovery details improve readability.

## Current limits

No changelists, shelving, amend, general interactive-rebase editor, conflict resolver, or force-push yet. Conflicts can be resolved externally; the app detects in-progress operations.

Hunk staging supports ordinary text modifications. Added, deleted, binary, symlink, submodule, and mode-changing files use whole-file actions. Text previews and filenames require UTF-8. Diff rendering is limited to 1 MB or 10,000 lines; untracked text previews to 2 MB. The backend still obtains full Git output before limiting rendering. Syntax highlighting supports a broad language catalog. Inline editing, line selection, and context expansion are not available.

History is paginated. Graph connections follow the unfiltered loaded history, including when a search hides rows. A native filesystem watcher covers the repository and linked-worktree metadata. A lightweight revision check runs each second; a 30-second status audit catches missed events (5 seconds if watching is unavailable). Command+R remains a recovery shortcut. The timeout and rebase-editor implementation currently uses Unix APIs. Apple Silicon macOS is the packaged target; Windows/Linux packaging has not been verified.

See [validation notes](../VALIDATION.md) for the checks performed on this version.

## Visual design

Version 0.2 uses near-black layered surfaces, readable system typography, restrained lavender graph accents, and soft-white primary buttons. The original branching-line logo is supplied as editable SVG and generated platform icons. See [visual design](../DESIGN.md) for the palette, design rationale, and links to the supplied inspiration.

## Appearance and simplified view (0.3)

Choose the sliders icon at the top right to open **Appearance**. Adjust interface text (12–18 px), diff text (12–20 px), compact/comfortable/relaxed spacing, and dark-mode contrast from soft charcoal to deep black. Changes preview immediately and are stored locally on this Mac. **Reset defaults** restores the starting settings.

**Simplified interface** is enabled by default. It removes duplicate navigation and headings, keeps hashes/dates in selected-commit details, moves selection help to a small hover/focus icon, and replaces the empty commit form with an All clear message. Turn it off in Appearance to show the denser navigation and metadata again.

The repository name is now the repository picker. The branch picker highlights on hover and keyboard focus. **Update** fetches and fast-forwards the current branch after destination review. The dropdown provides **Fetch only**, which uses the configured upstream remote (or the only remote) without pulling. With multiple remotes and no upstream, choose a remote first. Push stays a separate action.


## AI Connector (0.4)

Connect ChatGPT Desktop, Claude Code, Codex, Cursor, Claude Desktop, or another client once using the guided setup and reuse the same MCP configuration across local repositories. **AI requests → Connections** manages global clients, onboarding, optional source-diff access, and automatic review prompts. The AI submits an entire Git/GitHub command or shell chain for native approval. There is no subcommand allowlist or partial approval: approve, deny, or deny with a reason. Commands run unchanged through noninteractive `/bin/sh` using the user's account, including normal shell operators, hooks, helpers, and effects outside the initial repository.

Requests show the exact command and a plain-language AI note. Recognized commit, staging, branch, and pull-request arguments receive structured review cards: message/files, branch transitions, and PR title/description/source/base. Current metadata and prospective later effects are distinguished; dynamic expressions remain explicitly unknown. These details assist review without rewriting the command or certifying every effect.

Every execution records its connection, self-reported model, whole-command approval, timestamps, exact command, exit code, and timeout. Process success is distinct from verified effects. Raw output is withheld because it may contain secrets; command inputs remain in the private local journal. **Activity & commands** provides export and **Copy for AI**. **Open Branchline for review requests** can launch and foreground the request; it never approves it. The AI waits for the result, and timeout never means approval. Needs you lists pending approval decisions; blocked and completed requests appear in Activity. Clear activity removes local finished/blocked receipts after confirmation while preserving prepared and executing requests, connections and Git changes.

See [setup, privacy and execution limits](MCP_CONNECTOR.md) and [command coverage](COMMAND_COVERAGE.md). Installed tools, authentication, noninteractive execution, and timeouts determine whether a requested command can complete. Branchline is a permission gate, not a sandbox for an approved shell command.

## Reading and navigation

Diff revision labels, hunk count, and layout control share one quiet toolbar. Commit-selection help appears on hover or keyboard focus. Commit menus open at the pointer, have icons, remain inside the window, and support keyboard navigation. Branch search hides empty groups and suggests a named new branch when no result matches.

History browsing retains the current preview while its replacement loads, swaps the file list and diff together, ignores superseded responses, and caches recent immutable commit previews. Read navigation does not flash the mutation busy state.

Branch selection now opens contextual actions: switch/check out locally, push any local branch without switching, rename locally, create from the selected commit, copy name, and delete merged local branches. Push binds the selected branch tip and does not change upstream settings. Rename refuses branches open in other worktrees; deletion never forces unmerged history removal.

Review panels support persistent resizing and collapse controls. The commit sidebar labels its metadata and changed files; the Changes tab shows the changed-file count, with added/removed line totals in the footer.

The branch picker opens immediately from a repository-specific background cache. Click a branch (or right-click it) to open its action menu near the row. Background snapshots refresh after repository operations and refreshes; the picker’s Reload control or ⌘R rereads local refs without contacting the remote. A failed refresh retains the previous list.

Startup restores the last repository without first showing the welcome screen. A lightweight initial status opens the workspace; line totals and the staged-content fingerprint hydrate afterward, with commit submission disabled until a valid staged fingerprint is available. The window is revealed after its first frontend frame; MCP-only startup remains hidden.

Diffs now offer syntax token colors for Shiki’s 242 bundled languages, loaded on demand in a worker. Language detection covers common extensions and repository filenames, with a per-file language selector and Plain text fallback. Highlighting uses complete before/after source so multiline constructs retain their context; text is escaped and must match the displayed diff before colored spans are applied. Large/binary/unsupported files stay readable as plain text.

SVG, PNG, JPEG, GIF, WebP, AVIF, BMP, and ICO assets open in a Before/After image comparison. The panel-local Image / Code switch shows source diffs for SVG and Git metadata for raster images. Images are limited to 10 MB per version and cannot follow filesystem symlinks outside the repository. SVG is displayed as an image resource rather than injected into the app DOM.

Split diffs keep both versions visible, with a shared horizontal scroll control per side rather than per row. File-list position is retained when changing files within a selected commit. Panel resize handles have one-pixel dividers with wider invisible hit targets.

## Repository windows and AI skills

Use File → New Window (⌘N) to open a separate repository workspace. Each window has its own Git service, selection, diff, and panels. Opening a repository in a secondary window leaves the primary window's saved repository alone. Windows are independent during the session; restoring the full set of windows after app restart is not implemented.

The MCP connector, connections, request inbox, execution gate, and audit journal remain global. A review request focuses a matching repository window or creates a new window seeded with that repository. The AI's read-only inspection does not open a window. Approvals from any registered local app window go through the same native plan validation and execution queue. Every request keeps its own repository binding, regardless of the workspace visible underneath it.

On first launch with a detected supported client, Branchline offers the bundled `branchline-git-approval` skill. Nothing is installed until the user selects Install guide. Setup can be minimized and resumed; Not now records that the offer has been seen. Connections → AI skills opens it again. Connector onboarding also includes a final AI guide step, with a provider-specific install button or copy-only fallback. Installing the guide does not install an MCP connection or grant execution permission.

ChatGPT Desktop, Codex and Cursor use the shared `~/.agents/skills/branchline-git-approval/SKILL.md` location. Claude Code uses `~/.claude/skills/branchline-git-approval/SKILL.md`. Detection uses local settings directories and known application locations; cloud-only clients and unrecognized locations are not automatically installed. Shared locations may be discovered by other compatible clients. Existing custom files and symbolic-link destinations are preserved. Managed, unmodified installations update automatically in the background when the app starts. Customized guides stay untouched. To remove the guide manually, remove only the `branchline-git-approval` folder at the displayed installation path.

MCP is the execution boundary; skills describe the workflow. Skills do not prevent an AI from using shell commands outside Branchline. Client-specific hooks could be added as a separate, optional integration; none are silently configured by this installer.

Compatibility references: [Codex skills](https://learn.chatgpt.com/docs/build-skills), [Claude Code skills](https://code.claude.com/docs/en/skills), [Cursor skills](https://prod.cursor.com/docs/skills).

## Waiting for AI approval results

After `branchline_request_review`, call `branchline_wait_for_result` with the same `planId`. The optional `timeoutMs` defaults to 20000 and accepts 0–25000. The wait is read-only and does not raise a window, approve, execute, or cancel a plan. Native approval and execution remain available while a client waits.

The versioned result includes `waitOutcome`, current `status`, `timedOut`, `terminal`, `succeeded`, `grantsApproval: false`, the latest redacted `plan`, and `pullRequestUrl` when a PR step has succeeded with a verified URL. `finished` includes failed, cancelled, partially completed, and uncertain outcomes; check `succeeded` and each step rather than treating every finished wait as success. `needs_attention` returns for blocked plans, plans needing another review, or plans not yet submitted for review. `timed_out` means the request is still pending, queued, or running: wait again on the same ID. It never grants approval or implies completion. If the broker is busy past the deadline, the tool returns an explicit error instead of claiming an outcome it could not read.

Connection authorization, plan ownership, and source-content permissions are rechecked on each observation. Revocation or a transport failure is an error, not approval. The bundled skill instructs the AI to continue bounded waits while its task is active, report interruptions honestly, and include only a verified returned PR URL. Reconnect existing MCP clients to discover the new tool and reopen Branchline to update existing managed guides automatically (Connections → AI skills remains available for review or manual retries). A client or model can still stop its own turn; neither the tool nor the skill can force a host to remain active.

## Automatic guide maintenance

Each app launch checks previously installed Branchline guides once in the background. A matching ownership receipt and content hash are required before any automatic update. New installations still require the native consent flow; missing or removed guides are not recreated automatically. Symbolic-link destinations, modified or unmanaged guides, unsupported receipts, and newer installed revisions are preserved.

The updater saves the previous guide in the app-local `skill-backups` directory before replacing it atomically. A failed backup prevents replacement. Update results are recorded in `skill-updates.json` and summarized under AI Skills → Guide & install locations; failures retry on the next app launch. Existing current guides do not get rewritten on every startup. A running AI session may need to restart to load changed instructions. Updates use the guide bundled with the installed app, with no network downloads.

Maintainers: increase `REVISION` in `src-tauri/src/skills.rs` whenever the bundled skill changes. Legacy hash receipts are treated as revision 1. Revision ordering prevents an older app build from downgrading a newer managed guide; differing content with the same revision is preserved for manual review.
