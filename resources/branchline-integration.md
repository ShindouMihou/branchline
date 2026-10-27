## Git and GitHub approvals

Before changing Git state or GitHub resources, load the `branchline-git-approval` skill at `{{skill_path}}` and use the connected Branchline MCP server. This includes staging, commits, branches, history changes, pushes, and pull requests, including operations invoked by scripts or aliases.

Submit the complete command with `branchline_propose_command`, request native review, and wait with `branchline_wait_for_result`. Continue waiting after a timeout; timeout is not approval. Report the actual result and only verified pull request URLs.

Task instructions do not replace native approval. If Branchline is unavailable, pause Git/GitHub changes and tell the user; do not bypass review through a terminal, script, SDK, or another tool. Unrelated work can continue. Use Branchline inspection tools for supported read-only queries.
