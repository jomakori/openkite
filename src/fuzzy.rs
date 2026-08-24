//! Fuzzy matching for the command palette and global search.

/// A successful fuzzy match with its score and matched positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatch {
    /// Match strength; higher is better.
    pub score: i64,
    /// Indices of the `candidate` characters matched by the query.
    pub positions: Vec<usize>,
}

/// Whether a character acts as a word boundary in a resource/path identifier.
fn is_boundary(c: char) -> bool {
    c == ' ' || c == '-' || c == '_' || c == '.' || c == ':' || c == '/' || c == '\\'
}

/// Score how well `query` fuzzy-matches `candidate`.
///
/// A fuzzy match requires the query characters to appear in `candidate` as a
/// subsequence (case-insensitive). Scoring rewards word-boundary and
/// consecutive matches and lightly penalises long candidates and gaps.
/// Returns `None` when `query` is not a subsequence of `candidate`.
pub fn fuzzy_match(query: &str, candidate: &str) -> Option<FuzzyMatch> {
    if query.is_empty() {
        return Some(FuzzyMatch {
            score: 0,
            positions: Vec::new(),
        });
    }

    let needle: Vec<char> = query.to_lowercase().chars().collect();
    let haystack: Vec<char> = candidate.to_lowercase().chars().collect();

    let mut positions = Vec::with_capacity(needle.len());
    let mut score: i64 = 0;
    let mut qi = 0;
    let mut prev: Option<usize> = None;

    for (ci, &ch) in haystack.iter().enumerate() {
        if qi >= needle.len() {
            break;
        }
        if ch != needle[qi] {
            continue;
        }
        positions.push(ci);
        if ci == 0 || is_boundary(haystack[ci - 1]) {
            score += 8;
        }
        match prev {
            Some(pi) if pi + 1 == ci => score += 5,
            Some(_) => score -= 1,
            None => score += 4,
        }
        prev = Some(ci);
        qi += 1;
    }

    if qi < needle.len() {
        return None;
    }

    score -= haystack.len() as i64 / 16;
    Some(FuzzyMatch { score, positions })
}

/// Rank candidates by fuzzy score against `query`, best first, dropping
/// non-matches. Returns `(match, item)` pairs.
pub fn rank<'a, I, T>(query: &str, candidates: I) -> Vec<(FuzzyMatch, T)>
where
    I: IntoIterator<Item = (&'a str, T)>,
{
    let mut scored: Vec<(FuzzyMatch, T)> = candidates
        .into_iter()
        .filter_map(|(candidate, item)| fuzzy_match(query, candidate).map(|m| (m, item)))
        .collect();
    scored.sort_by_key(|(m, _)| std::cmp::Reverse(m.score));
    scored
}
