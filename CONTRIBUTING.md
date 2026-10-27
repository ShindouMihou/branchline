# Contributing to Branchline

Start with a reproducible issue or a focused change. For larger behavior changes, open an issue describing the user flow before building it.

## Development

Use macOS with Node.js 22+, npm, stable Rust, Xcode Command Line Tools, and Git.

```sh
npm ci
npm run desktop
```

Run the checks relevant to your change:

```sh
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

Use disposable repositories and local bare remotes for tests. GitHub CLI fixtures should stay offline unless a test explicitly documents its isolated remote and authorization. Never run test mutations against a personal working repository.

## Approval and command changes

Preserve the exact requested command. Approval belongs to the entire request and must come from the native user. A wait timeout grants no permission. Failed, cancelled, expired, or uncertain outcomes must remain distinguishable in the activity record.

Structured details assist review; they do not prove arbitrary shell effects. When context is dynamic or unsupported, explain the uncertainty and retain the exact command. Do not guess file membership, branch destinations, or pull request URLs.

Add regression coverage for changes to parsing, approval boundaries, execution, persistence, or result reporting. Keep read-only previews free of mutations and avoid reading unapproved source into MCP responses.

## Interface changes

Reuse the existing tokens, icons, file rows, diff renderer, and interaction patterns. Preserve keyboard navigation, focus, scroll position, empty states, and errors. Include a screenshot made with public or synthetic data when a visual change benefits from one.

Keep changes scoped. Avoid unrelated reformatting, new dependencies without a clear need, and comments that restate the code.

## Pull requests

Explain the problem, resulting behavior, and validation. Include any relevant limits. Do not include secrets, real activity journals, personal paths, or private source content in reports or screenshots.

Contributions are made under the repository's Apache License 2.0.
