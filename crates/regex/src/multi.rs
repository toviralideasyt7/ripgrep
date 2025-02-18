use aho_corasick::{AhoCorasick, MatchKind};
use grep_matcher::{Match, Matcher, NoError};
use regex_syntax::hir::{Hir, HirKind};

use crate::error::Error;
use crate::matcher::RegexCaptures;

/// A matcher for an alternation of literals.
///
/// Ideally, this optimization would be pushed down into the regex engine, but
/// making this work correctly there would require quite a bit of refactoring.
/// Moreover, doing it one layer above lets us do thing like, "if we
/// specifically only want to search for literals, then don't bother with
/// regex parsing at all."
#[derive(Clone, Debug)]
pub struct MultiLiteralMatcher {
    /// The Aho-Corasick automaton.
    ac: AhoCorasick,
}

impl MultiLiteralMatcher {
    /// Create a new multi-literal matcher from the given literals.
    pub fn new<B: AsRef<[u8]>>(
        literals: &[B],
    ) -> Result<MultiLiteralMatcher, Error> {
        let ac = AhoCorasick::builder()
            .match_kind(MatchKind::LeftmostFirst)
            .build(literals)
            .map_err(Error::generic)?;
        Ok(MultiLiteralMatcher { ac })
    }
}

impl Matcher for MultiLiteralMatcher {
    type Captures = RegexCaptures;
    type Error = NoError;

    fn find_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<Option<Match>, NoError> {
        match self.ac.find(&haystack[at..]) {
            None => Ok(None),
            Some(m) => Ok(Some(Match::new(at + m.start(), at + m.end()))),
        }
    }

    fn new_captures(&self) -> Result<RegexCaptures, NoError> {
        Ok(RegexCaptures::simple())
    }

    fn capture_count(&self) -> usize {
        1
    }

    fn capture_index(&self, _: &str) -> Option<usize> {
        None
    }

    fn captures_at(
        &self,
        haystack: &[u8],
        at: usize,
        caps: &mut RegexCaptures,
    ) -> Result<bool, NoError> {
        caps.set_simple(None);
        let mat = self.find_at(haystack, at)?;
        caps.set_simple(mat);
        Ok(mat.is_some())
    }

    // We specifically do not implement other methods like find_iter. Namely,
    // the iter methods are guaranteed to be correct by virtue of implementing
    // find_at above.
}

/// Alternation literals checks if the given HIR is a simple alternation of
/// literals, and if so, returns them. Otherwise, this returns None.
pub fn alternation_literals(expr: &Hir) -> Option<Vec<Vec<u8>>> {
    // This is pretty hacky, but basically, if `is_alternation_literal` is
    // true, then we can make several assumptions about the structure of our
    // HIR. This is what justifies the `unreachable!` statements below.

    if !expr.properties().is_alternation_literal() {
        return None;
    }
    let alts = match *expr.kind() {
        HirKind::Alternation(ref alts) => alts,
        _ => return None, // one literal isn't worth it
    };

    let mut lits = vec![];
    for alt in alts {
        let mut lit = vec![];
        match *alt.kind() {
            HirKind::Empty => {}
            HirKind::Literal(ref x) => lit.extend_from_slice(&x.0),
            HirKind::Concat(ref exprs) => {
                for e in exprs {
                    match *e.kind() {
                        HirKind::Literal(ref x) => lit.extend_from_slice(&x.0),
                        _ => unreachable!("expected literal, got {:?}", e),
                    }
                }
            }
            _ => unreachable!("expected literal or concat, got {:?}", alt),
        }
        lits.push(lit);
    }
    Some(lits)
}
