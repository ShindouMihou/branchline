# Optional automatic approval

Introduced in Branchline 0.5.0, 28 October 2026. Manual approval remains the default for new and existing installations. This feature does not change which Git/GitHub workflows should be routed through Branchline.

## Settings

Open **AI requests → Settings → Auto-approval → Add rule**. Choose one active connection and the current repository, **Choose another repository…** to browse for a local folder, or **All repositories**, then choose:

- **Selected operations:** stage literal file paths, create ordinary commits, push a specified branch, create pull requests, or edit PR titles/descriptions. Select several operations in one rule to allow matching whole chains.
- **All requests this session:** automatically authorize whole command requests from that connection with the selected repository scope. This is broad user-account command authority, including scripts and destructive commands. The starting repository is context, not a sandbox.

Add rule has two steps: choose the connection, repository scope, actions and required destinations; then review the summary and duration. Changing duration refreshes the native scope preview and clears consent. Review this scope, then explicitly check consent before enabling. The native preview resolves repository-specific scopes and push URLs; confirmation is bound to that scope, so changing the remote or repository identity requires another review. MCP cannot enable, widen, renew or resume rules. Existing requests, including prepared requests that have not yet requested review, remain manual.

All rules are held in broker memory. Choose **Until Branchline quits**, or 15, 30 or 60 minutes, whichever ends first. Time limits use a monotonic clock. Closing a repository window does not end the broker session; quitting/restarting Branchline does. Remembered rules across restarts are not implemented.

An **Auto** indicator and pause button appear next to AI Requests in the main toolbar. The requests modal also shows an active-rule banner with **Pause all**. Pause removes all rules and prevents unstarted policy-authorized work from starting. Add and confirm a new rule to resume; pausing cannot recall a shell chain already running. Individual rules can be removed in Settings. Disabling the connector or revoking a connection removes its authority.

## Selected-operation coverage

The authorization classifier is separate from the explanatory preview cards. It requires a complete, bounded literal command sequence. Every operation in the request must match one rule; no matching prefix runs separately. Different rules are not combined into a wider scope.

Initial supported forms:

| Operation | Required form and scope |
| --- | --- |
| Stage files | `git add -- path...` |
| Create commit | `git commit -m message`, including repeated `-m` / `--message` pairs; no amend, `-a`, hooks/config overrides or unknown flags |
| Push branch | `git push [-u\|--set-upstream] remote branch`, or `HEAD:refs/heads/branch` / `HEAD:branch` when HEAD's branch matches the destination |
| Create PR | `gh pr create --repo github.com/owner/name --head branch --base base --title title --body description`, optionally `--draft`; literal short aliases supported |
| Edit PR | `gh pr edit number --repo github.com/owner/name --title title --body description`; either title or body can be omitted |

Repository-specific push rules pin the remote's single push URL and exact destination branch. Global rules apply to all repositories for one selected connection, using each repository's configured remote with the chosen name and branch. Each automatically authorized request snapshots its actual repository and push URL; later URL changes stop execution. Global scope never covers other AI connections. Short source names must resolve to an existing local branch, so a same-named tag cannot inherit branch-push permission. No force, delete, mirror, multi-target, mismatched-branch push or implicit destination matches. Naming a primary branch in the rule explicitly includes that destination; it is visible in the confirmation.

PR rules pin the explicit GitHub.com repository; create rules also pin the base branch. Enterprise hosts and an implicit host (`--repo owner/name`) need manual review in this first version. Selected PR rules support `--body-file -` / `-F -` only when supplied stdin belongs to a single direct literal command. File-backed bodies and multi-command stdin ownership require manual review. Editing labels, reviewers, base branches, merging and closing are not covered by a title/description edit rule.

Small literal read-only checks can accompany operations: `git status` with recognized short/porcelain/branch flags, `git diff` with only cached/staged/check/quiet/exit-code/stat flags, `git branch --show-current`, and `git rev-parse HEAD`. Simple `&&`, semicolon and newline sequences are supported. Unknown options, aliases, scripts, environment assignments, redirects, pipelines, conditionals, command substitution and other dynamic shell expressions fall back to manual review. Commands are never rewritten to fit a rule.

These restrictions classify requested syntax and destinations, not every runtime effect of hooks, configuration or helpers. Enabling a rule explains that Git and GitHub executables still run with the user's account permissions. Broad session approval deliberately does not use the selected-operation classifier.

## Execution and activity

Only the first review request is evaluated for automatic authorization. Matching requests use the same native approval validation, exact-command digest, preview revalidation, queue and executor as manual approval. The broker rechecks rule identity, lifetime, connection, repository identity and scope at approval and execution boundaries. Changed previews still stop for another review with the existing specific explanation. Multiple windows share one broker and one execution claim.

Matching requests do not open review prompts. Requests that fail policy authorization return to manual review; pause, expiry or revocation cannot leave a queued automatic approval usable. A running shell command remains one indivisible command and may finish after pausing.

Activity records use `native_policy` authority for automatic approval and retain the rule ID/revision, native grant time, scope and expiry alongside the exact command, input digest and actual outcome. The snapshot remains understandable after the rule is removed. Live rules are never loaded from activity records. Raw stdin and source diffs remain omitted from exported receipts.

Agents always use propose → request review → wait for result. Automatic authorization is not success, and a timeout is never approval. The standard whole-request execution and result-verification limits still apply.
