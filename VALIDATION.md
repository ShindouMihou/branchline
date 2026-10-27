# Validation — 26 October 2026

Target: Apple Silicon macOS, packaged release version 0.3.

## Automated checks

- TypeScript checking and Vite production build passed.
- Seven frontend tests passed: diff parsing and line mapping, side-by-side pairing, special diff cases, merge/converging graph lanes, and a DOM integration flow for opening a repo and submitting a push dialog.
- Sixteen Rust integration tests passed against disposable real repositories.
- `cargo clippy --all-targets -- -D warnings` passed; sources formatted with rustfmt.
- npm audit reported zero known vulnerabilities.

Rust coverage includes partial staging/unstaging, commit and history, stale diffs, Unicode paths, unborn repositories, renames, binary files, missing final newlines, failed hooks, local/tracking branches, conflicting worktree checkout, fetch/pull/push against a real bare remote, divergent pull/push rejection, squash below HEAD with descendant replay, root squashing, older-commit rewording, backup refs, stale branch/plan rejection, published/dirty/nonconsecutive/merge rewrite rejection, pre-rebase hook failure, cherry-pick, revert, and conflict-abort restoration.

## Native UI verification

Using the packaged app and disposable repositories under work/:

- Opened a repository through the native folder picker.
- Reviewed a two-hunk diff, staged one hunk, committed it, and inspected its historical diff. The other hunk remained unstaged.
- Browsed the branch graph, selected two commits with Shift-click, previewed their squash, edited the combined message, and completed the squash.
- Switched local branches through the searchable branch picker.
- Pushed a feature branch to a local bare remote and established upstream tracking.
- Completed Fetch and fast-forward Pull from the native dialogs.
- Reworded an unpublished commit, observed the updated hash/message, expanded Recovery details, and successfully copied the backup ref.
- Confirmed the previously clipped banner is fully visible above the tool rail. Navigation and banners now occupy separate layout rows.
- Reviewed larger text, taller log rows, near-black panels, monochrome primary actions, graph accents, and the original vector logo in the native release app.

The visual overlap had a direct native screenshot repro; its fix was checked against the same banner/navigation scenario. The DOM test checks the layout structure, while native screenshots verify geometry. No pixel-layout assertion is claimed from the DOM-only test environment.

## Boundaries

Network tests used only local bare repositories. Live GitHub/GitLab authentication and provider-specific protection rules were not tested. Remote credential support is delegated to the user's configured Git helpers and SSH agent.

No existing user repository was modified. The locally ad-hoc signed app is not Apple-notarized. Windows/Linux builds were not tested. Backup refs remain in disposable test repositories and are excluded from the source archive.

## Appearance and simplified interface (0.3)

- Preference parsing tests cover corrupt storage, invalid types, bounds, and restored spacing.
- The DOM integration test changes interface and diff fonts, spacing, and contrast; verifies live CSS variables and saved storage; reopens Appearance; resets defaults; then completes the Update → Push flow.
- Native inspection confirmed the unified repository picker, hover-only branch control, simplified Changes/History navigation, quieter commit list, and Appearance sliders/radios.
- Appearance settings preview without rebuilding the DOM on slider input, preserving dragging and keyboard interaction. Supporting text has an 11 px minimum; the dialog scrolls when large fonts or smaller windows require it.
- Native keyboard interaction changed interface text from 14 px to 15 px immediately, and closing Appearance returned to the simplified workspace.

## Update split button and directional status

- The DOM integration flow opens Update options, reads the remote badge/outgoing indicator, runs Fetch only, and verifies that the only network action at that point is fetch (no pull).
- The same flow then reviews and submits Update (pull with Git's fast-forward-only behavior) and independently submits Push.
- Fetch only refreshes refs and history while preserving the selected commit/diff. Unknown tracking comparisons do not display invented counts.
- Escape and outside-click dismiss the dropdown; the dropdown button exposes expanded state and supports Arrow Down.
- Native Fetch only completed against the disposable local remote; independent Git inspection confirmed HEAD and the working tree were unchanged.


## Automatic local refresh

- Native filesystem watching observes the working tree, Git metadata, and external common/per-worktree metadata for linked worktrees. A real Git fixture test confirms repeated edits and a branch created from another worktree are detected.
- Frontend tests cover deferred/coalesced changes, late responses from a different repository, single in-flight checks, periodic audits, and the unavailable-watcher fallback.
- Native release inspection detected a new untracked file, refreshed its selected contents after a second edit with the same Git status, and returned to All clear after removal, without a manual refresh or focus trigger.
- Refresh toolbar control removed; Command+R retained. Automatic checks never perform network operations. Native user appearance preferences were preserved.
- Final suite: 17 Rust tests and 10 frontend tests; production TypeScript/Vite and Tauri release builds pass. App signature verified after packaging.

## AI Connector specification

- Product, architecture, and UX context were reviewed against the existing code.
- The private product brief was saved and read back. The repository engineering specification covers protocol compatibility, scope, exact approvals, operation coverage, native review, partial outcomes, credentials, and adversarial test gates.
- MCP and PR provider behavior is proposed, not implemented or interoperability-tested.


## MCP connector and visual review (0.4)

- Final automated suite: 27 Rust tests and 15 frontend tests. Production TypeScript/Vite and Tauri release builds pass.
- Real-repository broker tests cover native-only approval, stale staged content, ownership/token checks, global access to an unlisted repository, scope-preserving repository targets, source-read preference, idempotency, revocation, exact commit/push outcomes, prefix approval, remote races, hooks, journal persistence and restart recovery.
- Typed red-flag tests cover main/master destinations, mismatched branch names, rejection when any acknowledgement is missing, and absence of those flags for a same-name topic-branch push.
- Frontend tests exercise escaped AI explanations, explicit approval, target-warning gating, commit-only consent, clipboard receipt projection, default-on prompts, branded onboarding and project/global setup. Setup command generation tests preserve shell argument boundaries and client configuration formats.
- A stdio harness against the packaged executable verified initialization, advertised schemas, protocol-version rejection, inspection, proposal, review queueing, and no mutation before approval. Native user approval then completed commit → push against a disposable local bare remote; the receipt contains four execution/network commands with timestamps and verified results.
- With the desktop app fully quit, the paired adapter reopened it, inspected an absolute repositoryPath, queued a fresh request, and opened the native review directly. The request displayed both main-target and branch-mismatch flags, with publication disabled and no Git mutation.
- Native clipboard export displayed Copied for AI. The repaired dialog ACL opened the native Save panel and successfully exported the selected JSON activity record; the sample contains both completed steps and four recorded commands.
- Native inspection verified provider icons, connection classification without credential replacement, global/project Codex instructions, readable AI-note sections, connected step timeline and fixed approval footer.
- The history regression test first failed because the current diff node was detached immediately on selection. The fix keeps the prior preview during reads, publishes the next snapshot once, ignores superseded responses, and caches recent commit snapshots. Context-menu tests verify pointer positioning and six action icons; branch tests verify the actionable no-results state and readable source label.
- Tests used disposable fixtures under work/. Existing user repositories were not modified. Current review examples refer only to those fixtures. Host configuration files were not edited and setup commands were not installed into Claude/Codex/Cursor. Provider PR operations, live hosted-remote policies, and every host/version combination are not validated.

## Branch publication, GitHub CLI and request viewport update

- Real bare-remote tests create a previously absent branch only after native approval; a branch appearing after preview stops publication while retaining the completed commit.
- GitHub CLI subprocess fixtures verify explicit repo/head/base/draft arguments, multiline body stdin, executable-aware audit, verified returned PR fields, duplicate/head-change rejection, uncertain creation, and no blind retries. No hosted PR or external GitHub mutation was used in verification.
- Legacy command receipts default their executable to Git. Catalog tests distinguish supported scopes from manual commands.
- Frontend tests cover the PR timeline, escaped title/description, per-field clipboard behavior, and restoring expanded content before scroll offsets are clamped. View changes reset disclosure/scroll state; keyboard focus and inbox scroll survive same-view updates.

- Changes navigation shows a unique changed-file count. Footer line totals sum staged and unstaged diffs plus untracked text additions; binary files are excluded and large/unreadable untracked files show a partial-count indicator. Real-repository tests cover staged plus unstaged changes, binary files, untracked missing-final-newline text, and unborn repositories.

- The files sidebar and commit/diff split now use persistent pointer/keyboard dividers and collapse/restore controls. Saved dimensions are bounded, double-click resets a divider, and collapsing one review pane restores the other if necessary. Layout tests verify persisted-state validation and keyboard bounds.
- Removed the duplicate bottom selection-tip disclosure. The selection count sits beside the top-right help control; its tooltip wraps instead of inheriting nowrap.

- Selected commit metadata now separates author, full local date/time, copyable abbreviated hash, and changed files. Full hash/path remain available in tooltips; long file labels truncate inside their resizable panel.

- Branch-action tests push an unselected local branch to a new bare-remote destination without changing checkout/HEAD, reject stale branch tips and name collisions, preserve dirty files during current-branch rename, and reject deletion of current/unmerged branches.

- macOS bundle metadata points to a valid ICNS containing the Branchline mark. The app also explicitly sets its Dock image from the bundled PNG on the main thread at Ready, avoiding reliance on stale Launch Services icon lookup during repeated local builds.

## Branch menu and background snapshots

- Branch actions now use a row/pointer-anchored menu, with viewport clamping, keyboard navigation, Escape/outside dismissal, and no persistent bottom action grid. Branch-list scroll survives updates.
- Per-repository snapshots prefetch at repository open and refresh after Git operations, explicit refresh, and automatic repository refresh. In-flight requests coalesce; invalidation preserves visible cached data and rejects older completions.
- Branch snapshots carry the expected repository root. Rust rejects a late read against a different active repository. Current-branch identity is read once per branch list rather than once per branch.

## Startup and file preview update

- Quick-open integration tests assert an empty staged fingerprint and deferred totals until full status hydration, preventing premature commit submission. The frontend restoration test keeps the startup workspace visible while repository open is pending, with no welcome flash.
- Rust tests compare staged and committed SVG/source versions, validate MIME content, and reject traversing or external symlink paths. Frontend tests cover default image comparison, the panel-local Code switch, escaped SVG source, and raster diff metadata. No external image/grammar requests are required.
- Highlight tests use real Shiki TypeScript and Dart grammars, verify exact text round-tripping and escaped markup, cover extension detection, and check large-source fallback. Workers tokenize complete before/after source; small-file fallback covers unavailable workers.
- The commit-selection regression also verifies that selecting a different file retains the file-list scroll offset. Split diff rows no longer own overflow scroll containers; fixed split columns and one shared horizontal scroll control per version handle long lines.

Native macOS verification (2026-10-26): confirmed both split columns remain visible for a 600-character TSX line; syntax token colors render in WKWebView; SVG Before/After images and the panel-local Code switch work; PNG Before/After images load. Token colors are applied through DOM style properties to work with the bundled content security policy. Restored the app view to pedi-client-app after the disposable-fixture checks. Final checks: 27 frontend tests and 43 Rust tests pass; release build and ad-hoc signature verification pass.

Multi-window and skill onboarding (2026-10-26):
- Rust regressions cover independent actual repository services, matching-root routing selection, unknown/closed-window rejection, detection without installation, persisted dismissal, selected-target installation, custom-file preservation, and refusal of symlinked destinations. Installer writes in tests use temporary home directories.
- Frontend tests cover explicit consent, selected targets, guide escaping, minimize/resume without installation or dismissal, connector onboarding's AI guide step, and copy-only fallback for unsupported local skill clients.
- Native macOS checks confirmed ⌘N creates an independent workspace; a real MCP request focuses its matching fixture window; a second MCP request opens a previously unopened fixture in its own window. No approval or execution was performed. The executable test request was cancelled through MCP; the other fixture request stayed blocked with no staged files.
- Native scrolling verification used the reported AppDelegate.mm commit: line numbers and source now stay beneath the opaque sticky hunk header. The skill setup minimizes to a resumable chip. The user repository was restored without Git mutations.
- Final native preview confirms compact horizontal client rows with provider logos and right-aligned checkboxes. Final verification: 30 frontend tests and 48 Rust tests pass; release build and ad-hoc signature verification pass. Global skill folders were not modified by the verification run. The live installation prompt remains available for the user's own choice.

Connections and toolbar polish (2026-10-26): ChatGPT Desktop now selects the same agents skill target as Codex; the frontend regression verifies the shared installation path and explicit install control. Connections has a Requests back button, separate close control, and a main AI skills settings section. Native checks confirmed matching history-control heights and full-width collapsed-panel hover/expansion. The guide assurance uses aligned icon/text columns and a separate shared-folder info tooltip. New Window remains in File and on Command-N, with the extra top-bar button removed. All 30 frontend and 48 Rust tests pass.

Wait-for-result MCP (2026-10-27):
- Added a read-only, versioned structured-output tool, default wait 20 s, maximum 25 s; timeout never approves or executes. Native approval is not routed through MCP.
- Real local socket regressions cover pending timeout with no HEAD change, no commands, no approval, no journal events, and no window notification; invalid ranges/arguments; blocked plans; cross-client ownership; content redaction; revocation; and native decline during an active wait.
- The existing actual native-approved execution fixture also verifies a successful socket wait response. PR-result tests verify URL extraction only from successful PR steps, full success vs partial completion, and omission of unconfirmed URLs. No hosted PR is created by these tests.
- A busy broker cannot hold a waiter beyond its bounded lock-acquisition deadline; an unreadable state is reported as an error. Waiting does not hold the broker mutex between observations.
- Updated bundled guide and MCP initialization/tool instructions to wait on the same plan, distinguish timeout/approval/completion, avoid duplicate shell execution, and report actual per-step outcomes and verified PR URLs.
- Verification: 53 Rust tests and 30 frontend tests pass.
- Packaged-app stdio check: a fresh initialize → tools/list → tools/call session discovered the wait tool and output schema, then read an existing cancelled test plan as `finished`, `succeeded: false`, `grantsApproval: false`. It requested no new review or execution. Updated both existing unmodified managed global guides through the native installer; customized/unknown files were not replaced. The current previous workspace workspace was preserved without Git mutations.

Automatic skill maintenance (2026-10-27):
- The native app starts one background update check per process, independent of the number of repository windows. Previously installed managed guides update only when the receipt hash matches and the bundled revision is newer.
- Four update regressions cover legacy receipts, atomic replacement with verified backup, repeated launches without repeated rewrites, no first-time installation, deleted/unmanaged/customized preservation, no downgrade, same-revision mismatch, symbolic links, failed backup preservation, and successful retry after failure.
- Update receipts now include a monotonic skill revision; last-check results are recorded privately and exposed in AI Skills details. Existing current guide content is not rewritten just to migrate a legacy receipt.
- Verification: 57 Rust tests and 30 frontend tests pass; release build succeeds.
- Native startup check verified both existing managed guides were checked quietly, legacy receipts migrated to revision 2, and already-current SKILL.md content hashes and modification times remained unchanged. The persisted report recorded zero updates and zero issues. The user's active previous workspace repository view was retained without Git mutations.

Update menu layout (2026-10-27):
- Replaced oversized transfer cards with a branch-first summary and two aligned inline counts. Long tracking names truncate with the full name available on hover; the menu stays within the viewport.
- Fetch only uses a compact icon/text layout and the description “Check for updates without pulling.” Git behavior is unchanged.
- Verification: 30 frontend tests pass; release build and ad-hoc signature verification pass. Native macOS review confirmed the open dropdown's spacing, colors, alignment, and keyboard dismissal. No Git operations were triggered.

Whole-command approval and structured review (2026-10-27):
- The sole public proposal tool is branchline_propose_command. Exact shell text runs through /bin/sh only after native approval of the whole request. Partial approval is rejected; denial may include a reason returned in the plan. Legacy publication primitives remain for internal tests/historical receipts, not a public alternate execution mode.
- Four rounds of multi-agent implementation/review covered runner behavior, broker authorization, result privacy, discovery/skills, and visual review. Findings fixed include nonzero commands reported as success, output persisted into receipts, nullable stdin crashes, old prefix approval, misleading process-success labels, hidden-character display, and a false primary-branch warning for a normal PR base.
- Runner tests exercise literal arguments, unchanged shell chains/short-circuiting, bounded output and input, timeouts with process-group termination, descendant-held pipes, and replaced executable/directory rejection. Broker tests exercise pending requests without execution, full-scope risk acknowledgement, failed-command stopping, private output omission, denial reasons, idempotency, and the real public socket path.
- Structured commit/staging/branch/push/PR previews use bounded local inspection and conservative literal parsing, including projected git add . / -A / -u file selections. Tests cover quoted and multiline messages, body/message files, symlink/outside-path refusal for previews, implicit destinations, dynamic syntax, and changed shell context. Unsupported inspection never rewrites or prevents an otherwise valid command request.
- Native checks used a temporary unborn repository: the MCP request opened its own review window, preserved the exact command, displayed commit files/message and PR title/description/head/base, and showed the three decision buttons. Both visual requests were denied without executing any command; the denial reason was displayed and the original previous workspace window restored. No user repository Git actions were performed.
- The redesigned cards have operation icons, restrained colors, branch pills, file counts, and visible From command / Current snapshot / Projected labels. General warnings are compact supporting information. New request approval has no partial-action buttons or extra consent checkboxes; the native Approve action acknowledges the displayed whole-command scope.
- Final verification: 83 Rust tests and 35 frontend tests pass; production build succeeds. Command effects remain explicitly unverified; raw stdout/stderr are omitted from durable receipts. Installed tools, authentication, POSIX shell/noninteractive behavior, the 600-second maximum timeout, and request/input limits still apply. Managed guide revision is 3; existing MCP clients need to reconnect to discover the new tool.

Inspectable approval changes and UX assessment (2026-10-27):
- Stage/Commit cards expose Review changes actions and full clickable file rows. The same request contains a read-only diff subview with file navigation, line numbers, change totals, loading/error/retry states, and Back/Escape navigation. Approval remains for the entire command.
- Native review is restricted to files and sources captured in the selected detail. It releases the broker mutex before Git/filesystem work, verifies repository identity, uses bounded immutable Git blobs and no-follow worktree reads, and performs comparisons on private temporary snapshots without external diff/textconv or lazy fetching. Loaded patches are not journaled, exported, or exposed through MCP.
- Scope is typed: Stage compares index → worktree; current Commit compares HEAD → index; a prospective Commit uses HEAD → worktree for earlier staging selections and index content for unrelated staged files. Snapshot caveats and binary/oversized/mode/rename limits are explicit.
- Regressions cover partially staged content, new/deleted/binary files, traversal/symlinks, source membership, native-only access, unchanged index/HEAD/journal, content resembling diff headers, stale async responses, inline errors, preserved scrolling and originating focus. Full checks: 88 Rust tests and 37 frontend tests pass, and the release build succeeds.
- Native fixture walkthrough verified both Stage and Commit diff entry points, file switching, correct added-line content, Back/Escape focus restoration, and whole-command decisions. The test request was denied, the fixture index stayed empty, and the original previous workspace window was restored. No proposed command or user-repository Git mutation ran.
- The expert UX assessment is in docs/APPROVAL_UX_ASSESSMENT.md. Implemented improvements include human inbox titles, linked action overview, inspectable files, typed certainty, and a compact exact-command disclosure; the redundant single-command heading is hidden. This is an expert/fixture assessment, not a participant study or measured usability score.

Lighter approval cards (2026-10-27):
- Retained individual step cards per user preference, with softer borders, smaller icon tiles, quieter labels, and ordinary UI text for file names. One compact file-count/View diff row replaces the oversized Review changes control. Back controls use accessible chevrons, and snapshot context uses an info disclosure instead of a permanent paragraph above the diff.
- When Stage and the next Commit contain the same file set (regardless of order), files appear only under Commit. Stage retains a compact explanation and its clickable heading still opens the staging comparison. Different selections remain separate. Regression coverage verifies both cases.
- All 38 frontend tests pass. Native review of an existing denied fixture confirms the shared file list, working Stage heading, icon-only back navigation, unchanged diff content, and collapsed snapshot disclosure. No requests were approved or executed and the user repository stayed open.

Main-view consistency for approval review (2026-10-27):
- Approval step cards and the review file sidebar use the main view's purple code-file icon and shared filename/directory labels. Individual operation cards and Stage/Commit file deduplication remain intact.
- The main and approval viewers share escaped split/unified hunk rendering and syntax token application. The native-only review response supplies bounded snapshot source for highlighting; approval review has no stage/unstage controls. Revision labels, hunk accents, change colors, quiet layout/language controls and per-side horizontal scrolling follow the main viewer.
- All 42 frontend tests and 88 Rust tests pass; four focused native review tests also verify snapshot source values. Coverage includes matching token text, escaped source, explicit mutation control injection only for the main viewer, layout switching and metadata-only empty states.
- Final native walkthrough confirmed purple card/list icons, aligned single-row revision controls, colored totals, split/unified switching, and subsequent file selection retaining unified mode. Inspected the already-denied fixture only; no approval or proposed command was executed. Release app rebuilt and signature verified.

Stable approval file icons (2026-10-27):
- Reproduced the added-file icon replacement with a failing regression test: after a native snapshot loaded, the first row lost its file icon while untouched rows kept theirs. Removed the per-file status substitution from the approval sidebar so every row keeps the same purple file icon before and after loading or selection.
- All 43 frontend tests pass, including added-file loading and subsequent file selection. No native execution or permission logic changed.

Full-width single-version review (2026-10-27):
- Added and deleted file snapshots use full-width unified rows, avoiding an empty comparison column. The revision toolbar labels these New file / Deleted file and omits the unnecessary layout toggle; modified files retain the existing split/unified preference.
- Regression coverage checks both single-version cases and returning to a modified file. All 44 frontend tests pass.

Blind multi-agent command testing (2026-10-27):
- Three independent agents added 22 executor, preview and permission-gate regressions; a separate public-connector compound-workflow pass added 2. Full results: 112 Rust tests and 44 frontend tests pass. Packaged MCP stdio checks also passed for all supported protocol versions and malformed/oversized/private-file boundaries.
- Fixed three structured-parser inaccuracies around Git/gh option terminators and branch flags, plus preservation of failed/outcome_unknown terminal outcomes on late cancellation. Exact approved command strings remain unchanged.
- Verified local commit/push workflows and controlled offline gh success/failure, not actual GitHub publication. Confirmed detached/background subprocess limits and clarified timeout UI copy. See docs/BLIND_COMMAND_TEST_REPORT.md for scenarios, evidence and limits.

Eight additional blind cycles (2026-10-27):
- Three independent agents completed eight new numbered cycles each; a packaged stdio harness completed eight protocol/credential cycles. Added 26 Rust regressions, including actual local Git index/message oracles, queued cancellation/revocation/expiry, concurrent waits, subprocess exits and exact arguments.
- Fixed restaging projection that could incorrectly retain a reverted staged file, using explicit unknown file scope where simulation is unsafe. Fixed the queued-worker error handler overwriting a terminal cancellation with needs_review.
- Final verification: 138 Rust tests and 44 frontend tests pass, plus all eight wire cycles against the rebuilt, signature-verified packaged app. Details and reproducible harness: docs/BLIND_COMMAND_TEST_REPORT.md and scripts/test-mcp-wire.mjs. All mutations confined to disposable fixtures; GitHub remains offline.

Request activity cleanup and quieter review (2026-10-27):
- Moved repository/time-limit/account-permissions information into the collapsed Exact command section and removed the repeated Runs in/outcome paragraph. Publishing red flags stay visible.
- Needs you now contains only pending-review requests. Blocked historical plans move to Activity with Cannot run wording, and empty inboxes show an appropriate empty state.
- Activity provides confirmed, native-only global history clearing. Backend keeps prepared/pending/approved/running records, credentials and Git state; it persists removal atomically and restores memory on write failure. Cleared approvals cannot be replayed, and a reused idempotency key requires a new request/approval. Exports remain unchanged.
- Tests cover record classification, confirmation/cancel/error/retry UI, preservation of a pending approval and running request, native-only deletion, persistence, failure rollback and replay prevention.
- Final verification: all 140 Rust tests and 46 frontend tests pass; release build and signature verification succeed. Native app confirmed the corrected Needs you and Activity empty states. Clear confirmation, cancellation and failure/retry were exercised in UI fixtures; no automated native clear-history confirmation was clicked.
