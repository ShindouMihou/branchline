use serde::Serialize;
use std::collections::VecDeque;

const RETAINED_COMMANDS: usize = 8;
const STREAM_LIMIT: usize = 131_072;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Snapshot {
    pub step_index: usize,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Default)]
pub(super) struct OutputStore {
    entries: VecDeque<(String, Snapshot)>,
}

impl OutputStore {
    pub fn update(
        &mut self,
        id: &str,
        step_index: usize,
        out: &[u8],
        err: &[u8],
        out_cut: bool,
        err_cut: bool,
    ) {
        let snapshot = Snapshot {
            step_index,
            stdout: String::from_utf8_lossy(&out[out.len().saturating_sub(STREAM_LIMIT)..])
                .into_owned(),
            stderr: String::from_utf8_lossy(&err[err.len().saturating_sub(STREAM_LIMIT)..])
                .into_owned(),
            stdout_truncated: out_cut || out.len() > STREAM_LIMIT,
            stderr_truncated: err_cut || err.len() > STREAM_LIMIT,
        };
        if let Some((_, previous)) = self
            .entries
            .iter_mut()
            .find(|(plan, entry)| plan == id && entry.step_index == step_index)
        {
            *previous = snapshot;
        } else {
            self.entries.push_back((id.into(), snapshot));
            while self.entries.len() > RETAINED_COMMANDS {
                self.entries.pop_front();
            }
        }
    }

    pub fn snapshots(&self, id: &str) -> Vec<&Snapshot> {
        self.entries
            .iter()
            .filter(|(plan, _)| plan == id)
            .map(|(_, output)| output)
            .collect()
    }

    pub fn retain(&mut self, ids: &[String]) {
        self.entries.retain(|(id, _)| ids.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_output_is_isolated_bounded_and_cleared_with_activity() {
        let mut store = OutputStore::default();
        for index in 0..10 {
            store.update(
                &index.to_string(),
                0,
                b"progress",
                b"diagnostic",
                false,
                true,
            );
        }
        assert!(store.snapshots("0").is_empty());
        assert_eq!(store.snapshots("9")[0].stdout, "progress");
        assert!(store.snapshots("9")[0].stderr_truncated);
        store.update("9", 0, b"finished", b"", false, false);
        assert_eq!(store.snapshots("9").len(), 1);
        assert_eq!(store.snapshots("9")[0].stdout, "finished");
        store.retain(&["8".into()]);
        assert!(store.snapshots("9").is_empty());
        assert_eq!(store.snapshots("8").len(), 1);
    }
}
