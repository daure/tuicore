#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Contains,
    Fuzzy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub score: i64,
    pub spans: Vec<MatchSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedSearchMatch {
    pub index: usize,
    pub score: i64,
    pub spans: Vec<MatchSpan>,
}

pub fn search_match(query: &str, candidate: &str, mode: SearchMode) -> Option<SearchMatch> {
    match mode {
        SearchMode::Contains => contains_match(query, candidate),
        SearchMode::Fuzzy => fuzzy_match(query, candidate),
    }
}

pub fn search_ranked<I, S>(query: &str, candidates: I, mode: SearchMode) -> Vec<RankedSearchMatch>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut matches = candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            search_match(query, candidate.as_ref(), mode).map(|matched| RankedSearchMatch {
                index,
                score: matched.score,
                spans: matched.spans,
            })
        })
        .collect::<Vec<_>>();

    if mode == SearchMode::Fuzzy {
        matches.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.index.cmp(&right.index))
        });
    }

    matches
}

fn contains_match(query: &str, candidate: &str) -> Option<SearchMatch> {
    if query.is_empty() {
        return Some(SearchMatch {
            score: 0,
            spans: Vec::new(),
        });
    }

    let query = query.chars().collect::<Vec<_>>();
    let candidate_chars = candidate.char_indices().collect::<Vec<_>>();
    if query.len() > candidate_chars.len() {
        return None;
    }

    for start in 0..=candidate_chars.len().saturating_sub(query.len()) {
        let matched = query.iter().enumerate().all(|(offset, expected)| {
            chars_eq_ignore_case(candidate_chars[start + offset].1, *expected)
        });
        if matched {
            let start_byte = candidate_chars[start].0;
            let end_index = start + query.len();
            let end_byte = candidate_chars
                .get(end_index)
                .map(|(index, _)| *index)
                .unwrap_or(candidate.len());
            return Some(SearchMatch {
                score: query.len() as i64,
                spans: vec![MatchSpan {
                    start: start_byte,
                    end: end_byte,
                }],
            });
        }
    }

    None
}

fn fuzzy_match(query: &str, candidate: &str) -> Option<SearchMatch> {
    if query.is_empty() {
        return Some(SearchMatch {
            score: 0,
            spans: Vec::new(),
        });
    }

    let query = query.chars().collect::<Vec<_>>();
    let candidate_chars = candidate.char_indices().collect::<Vec<_>>();
    let mut query_index = 0;
    let mut matched_indices = Vec::with_capacity(query.len());

    for (candidate_index, (_, candidate_char)) in candidate_chars.iter().enumerate() {
        if chars_eq_ignore_case(*candidate_char, query[query_index]) {
            matched_indices.push(candidate_index);
            query_index += 1;
            if query_index == query.len() {
                break;
            }
        }
    }

    if query_index != query.len() {
        return None;
    }

    Some(SearchMatch {
        score: fuzzy_score(&candidate_chars, &matched_indices),
        spans: match_spans(candidate, &candidate_chars, &matched_indices),
    })
}

fn fuzzy_score(candidate: &[(usize, char)], matched_indices: &[usize]) -> i64 {
    let mut score = 0;
    let first = matched_indices.first().copied().unwrap_or(0);

    for (position, index) in matched_indices.iter().copied().enumerate() {
        score += 10;
        if position > 0 && matched_indices[position - 1] + 1 == index {
            score += 12;
        }
        if is_boundary(candidate, index) {
            score += 8;
        }
        score -= index as i64;
    }

    score - first as i64
}

fn match_spans(
    candidate: &str,
    candidate_chars: &[(usize, char)],
    matched_indices: &[usize],
) -> Vec<MatchSpan> {
    let mut spans = Vec::new();
    let Some(mut span_start_index) = matched_indices.first().copied() else {
        return spans;
    };
    let mut previous = span_start_index;

    for index in matched_indices.iter().copied().skip(1) {
        if previous + 1 == index {
            previous = index;
            continue;
        }
        spans.push(span_from_indices(
            candidate,
            candidate_chars,
            span_start_index,
            previous,
        ));
        span_start_index = index;
        previous = index;
    }

    spans.push(span_from_indices(
        candidate,
        candidate_chars,
        span_start_index,
        previous,
    ));
    spans
}

fn span_from_indices(
    candidate: &str,
    candidate_chars: &[(usize, char)],
    start: usize,
    end: usize,
) -> MatchSpan {
    MatchSpan {
        start: candidate_chars[start].0,
        end: candidate_chars
            .get(end + 1)
            .map(|(index, _)| *index)
            .unwrap_or(candidate.len()),
    }
}

fn is_boundary(candidate: &[(usize, char)], index: usize) -> bool {
    if index == 0 {
        return true;
    }

    let previous = candidate[index - 1].1;
    let current = candidate[index].1;
    !previous.is_alphanumeric()
        || (previous.is_lowercase() && current.is_uppercase())
        || previous == '_'
}

fn chars_eq_ignore_case(left: char, right: char) -> bool {
    left.to_lowercase().eq(right.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_matches_case_insensitively_and_preserves_order() {
        let matches = search_ranked(
            "ap",
            ["Grape", "apple", "Paper", "pear"],
            SearchMode::Contains,
        );

        assert_eq!(
            matches
                .iter()
                .map(|matched| matched.index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(matches[0].spans, vec![MatchSpan { start: 2, end: 4 }]);
    }

    #[test]
    fn fuzzy_requires_subsequence_and_returns_match_spans() {
        let matched =
            search_match("fb", "foo_bar", SearchMode::Fuzzy).expect("subsequence should match");

        assert_eq!(
            matched.spans,
            vec![
                MatchSpan { start: 0, end: 1 },
                MatchSpan { start: 4, end: 5 },
            ]
        );
        assert!(search_match("fz", "foo_bar", SearchMode::Fuzzy).is_none());
    }

    #[test]
    fn fuzzy_ranks_consecutive_boundary_and_early_matches() {
        let matches = search_ranked("ab", ["xxab", "a-b", "zz_a_b", "ab"], SearchMode::Fuzzy);

        assert_eq!(
            matches
                .iter()
                .map(|matched| matched.index)
                .collect::<Vec<_>>(),
            vec![3, 1, 0, 2]
        );
    }

    #[test]
    fn empty_query_matches_without_spans() {
        let matched =
            search_match("", "anything", SearchMode::Contains).expect("empty query should match");

        assert_eq!(matched.score, 0);
        assert!(matched.spans.is_empty());
    }
}
