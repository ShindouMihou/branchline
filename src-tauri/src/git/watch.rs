use super::*;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

#[derive(Default)]
pub(super) struct RepoWatch {
    watcher: Option<RecommendedWatcher>,
    revision: Arc<AtomicU64>,
    healthy: Arc<AtomicBool>,
}
impl RepoWatch {
    pub(super) fn start(root: &Path) -> Self {
        let mut state = Self::default();
        let revision = state.revision.clone();
        let healthy = state.healthy.clone();
        let result =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
                Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                    revision.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => {
                    healthy.store(false, Ordering::Relaxed);
                    revision.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            });
        if let Ok(mut watcher) = result {
            let mut roots = vec![root.to_path_buf()];
            // Linked worktrees keep refs and their index outside the working directory.
            if let Ok(raw) = run(
                root,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
                None,
            ) {
                let common = PathBuf::from(String::from_utf8_lossy(&raw).trim());
                if !common.starts_with(root) {
                    roots.push(common);
                }
            }
            if let Ok(raw) = run(root, &["rev-parse", "--absolute-git-dir"], None) {
                let gitdir = PathBuf::from(String::from_utf8_lossy(&raw).trim());
                if !roots.iter().any(|p| gitdir.starts_with(p)) {
                    roots.push(gitdir);
                }
            }
            if roots
                .iter()
                .all(|path| watcher.watch(path, RecursiveMode::Recursive).is_ok())
            {
                state.healthy.store(true, Ordering::Relaxed);
                state.watcher = Some(watcher);
            }
        }
        state
    }
    pub(super) fn revision(&self) -> String {
        self.revision.load(Ordering::Relaxed).to_string()
    }
    pub(super) fn healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait_for_change(watch: &RepoWatch, before: String) {
        let start = std::time::Instant::now();
        while watch.revision() == before && start.elapsed() < std::time::Duration::from_secs(6) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert_ne!(
            watch.revision(),
            before,
            "filesystem change was not observed"
        );
    }
    #[test]
    fn watches_worktree_edits_and_external_git_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("main");
        std::fs::create_dir(&root).unwrap();
        run(&root, &["init", "-b", "main"], None).unwrap();
        run(
            &root,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "Initial",
            ],
            None,
        )
        .unwrap();
        let linked = dir.path().join("linked");
        run(
            &root,
            &["worktree", "add", "-b", "linked", linked.to_str().unwrap()],
            None,
        )
        .unwrap();
        let watch = RepoWatch::start(&linked);
        assert!(watch.healthy());
        let before = watch.revision();
        std::fs::write(linked.join("sample.txt"), "one").unwrap();
        wait_for_change(&watch, before);
        let before = watch.revision();
        std::fs::write(linked.join("sample.txt"), "two").unwrap();
        wait_for_change(&watch, before);
        let before = watch.revision();
        run(&root, &["branch", "external"], None).unwrap();
        wait_for_change(&watch, before);
    }
}
