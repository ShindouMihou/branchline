## Git and GitHub approvals

Branchline applies strictly to Git and GitHub effects, including those invoked through scripts or aliases. Unrelated work such as Convex configuration, application deployment, database/cloud changes, or non-GitHub authentication/security changes uses that service's normal tools and permissions, even when remote or destructive. Being inside a Git repository does not change this scope. Branchline is not a fallback for unknown commands; inspect or clarify their effects first.

Use normal tools for read-only Git/GitHub queries, builds, tests, and safe local Git work: fetch without force/pruning, staging/unstaging without moving HEAD or discarding files, stash save/apply/pop with conflict protections, and local branch creation/rename/switching that preserves work. Disposable isolated fixture tests may commit, rewrite history, and push to local fixture remotes without Branchline; they must not use real credentials/remote services or affect user repositories/shared resources.

For commits in real repositories, Git pushes/remote writes, GitHub resource changes (including PRs/releases), Git history rewrites/deletions/discarding work, or Git/GitHub authentication, repository-security or Git-publication-destination changes, load `branchline-git-approval` at `{{skill_path}}` and use Branchline. Preserve stricter explicit user/repository requirements for Git/GitHub work.

A gated Git/GitHub workflow needs whole-command approval, including supporting checks/setup. Keep unrelated service mutations in separately authorized normal-tool operations when composing work. If a user supplies a chain mixing a gated Git/GitHub action with unrelated service mutations, explain the scope conflict and ask before splitting/changing it; do not route unrelated mutations wholesale through Branchline or silently rewrite the command.

For gated work, use `branchline_propose_command`, request native review, and wait with `branchline_wait_for_result`. A timeout is never approval; continue waiting and report the actual result and only verified PR URLs. Task authorization does not replace native approval. If Branchline is unavailable, pause gated Git/GitHub work; other authorized work can continue.

Manual review is the default. Users may opt into scoped, session-limited automatic approval in Branchline. Always submit gated work through the same propose/review/wait workflow; never enable a rule, assume a match, or bypass Branchline because a previous request was auto-approved. Report the actual result.
