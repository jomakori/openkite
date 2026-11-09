//! The log panel's data path, shared by both targets of this crate.
//!
//! The panel's follow rule lives in `openkite_api::pod::FollowTail`; what a host
//! adds on top of it is turning the cluster's log endpoint text into lines and
//! keeping the panel's buffer in step with the bytes the endpoint has sent so
//! far. Both pieces are pure, so a test can drive a real stream (`/api/logs`)
//! without a browser (OKT-189).

/// Split the log endpoint's text body into the lines the panel paints.
///
/// kube's log endpoint answers one record per line terminated by `\n`, and a
/// partial record at the tail is normal while a container still writes. The
/// trailing empty segment the final newline produces is dropped, so an empty
/// line in the panel always means the log really had one.
pub fn lines_from_text(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect();
    if lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

/// The panel's view of a log stream that keeps being re-read.
///
/// kube has no cursor for a finished read: the host asks for the pod's log
/// again and gets every line it already painted. [`LogFeed::merge`] keeps only
/// what the stream added since the last read, so the panel appends instead of
/// duplicating, and a stream that has not grown adds nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogFeed {
    /// How many lines of the stream the panel has painted already.
    seeded: usize,
}

impl LogFeed {
    /// A feed with the first `seeded` lines already painted (the SSR pass ships
    /// them in the snapshot, so the client starts in step with them).
    pub fn seeded(seeded: usize) -> Self {
        Self { seeded }
    }

    /// How many lines of the stream are painted.
    pub fn painted(&self) -> usize {
        self.seeded
    }

    /// Take a full read of the stream and return the lines it added, in order.
    ///
    /// A read shorter than what is painted means the endpoint restarted the
    /// stream (a new container, or a truncated log): the read is the whole
    /// stream again, so all of it is new.
    pub fn merge(&mut self, read: Vec<String>) -> Vec<String> {
        if read.len() < self.seeded {
            self.seeded = read.len();
            return read;
        }
        let new = read[self.seeded..].to_vec();
        self.seeded = read.len();
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_final_newline_does_not_paint_an_empty_line() {
        assert_eq!(
            lines_from_text("2026-11-09T00:00:00Z Starting\ntail line\n"),
            vec![
                "2026-11-09T00:00:00Z Starting".to_string(),
                "tail line".to_string()
            ]
        );
    }

    #[test]
    fn crlf_and_blank_lines_survive() {
        assert_eq!(
            lines_from_text("a\r\n\r\nb"),
            vec!["a".to_string(), String::new(), "b".to_string()]
        );
    }

    #[test]
    fn an_empty_read_has_no_lines() {
        assert!(lines_from_text("").is_empty());
    }

    #[test]
    fn a_repeated_read_adds_nothing() {
        let mut feed = LogFeed::default();
        let read = lines_from_text("one\ntwo\n");
        assert_eq!(feed.merge(read.clone()), vec!["one", "two"]);
        assert_eq!(feed.merge(read), Vec::<String>::new());
        assert_eq!(feed.painted(), 2);
    }

    #[test]
    fn a_grown_read_adds_only_what_the_stream_added() {
        let mut feed = LogFeed::seeded(2);
        assert_eq!(
            feed.merge(lines_from_text("one\ntwo\nthree\n")),
            vec!["three"]
        );
        assert_eq!(feed.painted(), 3);
        assert_eq!(
            feed.merge(lines_from_text("one\ntwo\nthree\nfour\nfive\n")),
            vec!["four", "five"]
        );
    }

    #[test]
    fn a_restarted_stream_repaints_from_the_start() {
        let mut feed = LogFeed::seeded(3);
        assert_eq!(feed.merge(lines_from_text("fresh\n")), vec!["fresh"]);
        assert_eq!(feed.painted(), 1);
    }
}
