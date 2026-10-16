//! Word-by-word comparison of a dictation against the original sentence.
//!
//! Case and punctuation are ignored, and contractions are expanded on both
//! sides so that "it's" matches "it is", "don't" matches "do not", etc.

/// How a typed word relates to the reference sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    /// Matches a word of the sentence.
    Ok,
    /// Typed in place of a sentence word that was missed (misheard).
    Wrong,
    /// Not in the sentence at all: typed in addition to what was said.
    Extra,
}

/// The result of comparing a typed sentence with the reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    /// Reference words as written, and whether each was heard correctly.
    pub reference: Vec<(String, bool)>,
    /// Typed words as written, and how each relates to the reference.
    pub typed: Vec<(String, Typed)>,
}

impl Diff {
    /// (correct, total) over reference words that contain letters or digits.
    pub fn score(&self) -> (usize, usize) {
        let words: Vec<_> = self
            .reference
            .iter()
            .filter(|(w, _)| !tokens(w).is_empty())
            .collect();
        (words.iter().filter(|(_, ok)| *ok).count(), words.len())
    }

    /// Typed words that have no counterpart in the sentence.
    pub fn extra_words(&self) -> Vec<&str> {
        self.typed
            .iter()
            .filter(|(_, t)| *t == Typed::Extra)
            .map(|(w, _)| w.as_str())
            .collect()
    }
}

/// One normalized token: the spellings that count as equal to it.
type Token = Vec<String>;

/// A normalized token together with the index of the word it came from.
struct Tok {
    alts: Vec<String>,
    word: usize,
}

pub fn compare(reference: &str, typed: &str) -> Diff {
    let ref_words: Vec<&str> = reference.split_whitespace().collect();
    let typed_words: Vec<&str> = typed.split_whitespace().collect();
    let r = flatten(&ref_words);
    let t = flatten(&typed_words);
    let pairs = lcs(&r, &t);
    let mut r_hit = vec![false; r.len()];
    for &(i, _) in &pairs {
        r_hit[i] = true;
    }
    let t_class = classify_typed(&pairs, r.len(), t.len());
    let reference = mark(&ref_words, &r, &r_hit);
    let typed = typed_words
        .iter()
        .enumerate()
        .map(|(w, text)| {
            let classes = t.iter().zip(&t_class).filter(|(tok, _)| tok.word == w);
            let mut status = Typed::Ok;
            for (_, &c) in classes {
                status = match (status, c) {
                    (_, Typed::Wrong) | (Typed::Wrong, _) => Typed::Wrong,
                    (_, Typed::Extra) | (Typed::Extra, _) => Typed::Extra,
                    _ => Typed::Ok,
                };
            }
            (text.to_string(), status)
        })
        .collect();
    Diff { reference, typed }
}

/// Classifies every typed token. Between two consecutive matched pairs, the
/// unmatched typed tokens first stand in for the unmatched reference tokens
/// of the same stretch (misheard); any beyond that were typed in addition.
fn classify_typed(pairs: &[(usize, usize)], n_ref: usize, n_typed: usize) -> Vec<Typed> {
    let mut class = vec![Typed::Ok; n_typed];
    // Sentinels before the first and after the last match.
    let mut bounds: Vec<(isize, isize)> = vec![(-1, -1)];
    bounds.extend(pairs.iter().map(|&(i, j)| (i as isize, j as isize)));
    bounds.push((n_ref as isize, n_typed as isize));
    for w in bounds.windows(2) {
        let ((pi, pj), (ni, nj)) = (w[0], w[1]);
        let mut missed = (ni - pi - 1).max(0);
        for j in (pj + 1)..nj {
            class[j as usize] = if missed > 0 {
                missed -= 1;
                Typed::Wrong
            } else {
                Typed::Extra
            };
        }
    }
    class
}

fn flatten(words: &[&str]) -> Vec<Tok> {
    words
        .iter()
        .enumerate()
        .flat_map(|(i, w)| tokens(w).into_iter().map(move |alts| Tok { alts, word: i }))
        .collect()
}

/// A word is correct when every token it produced was matched. Words that
/// produce no tokens (stray punctuation) count as correct.
fn mark(words: &[&str], toks: &[Tok], hit: &[bool]) -> Vec<(String, bool)> {
    words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let ok = toks
                .iter()
                .zip(hit)
                .filter(|(t, _)| t.word == i)
                .all(|(_, h)| *h);
            (w.to_string(), ok)
        })
        .collect()
}

/// Longest common subsequence; returns the matched (a, b) index pairs in order.
fn lcs(a: &[Tok], b: &[Tok]) -> Vec<(usize, usize)> {
    let eq = |x: &Tok, y: &Tok| x.alts.iter().any(|s| y.alts.contains(s));
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if eq(&a[i], &b[j]) {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if eq(&a[i], &b[j]) && dp[i][j] == dp[i + 1][j + 1] + 1 {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// Words whose `'s` means "is"/"has" rather than a possessive.
const S_IS: &[&str] = &[
    "it",
    "he",
    "she",
    "that",
    "what",
    "there",
    "here",
    "who",
    "where",
    "how",
    "when",
    "why",
    "this",
    "everything",
    "nothing",
    "something",
    "someone",
    "somebody",
    "everyone",
    "everybody",
    "nobody",
    "anything",
    "anybody",
];

/// Splits one written word into normalized tokens with accepted alternatives.
fn tokens(word: &str) -> Vec<Token> {
    let lower = word
        .to_lowercase()
        .replace(['\u{2019}', '\u{2018}', '`'], "'");
    let mut out = Vec::new();
    // Hyphens and dashes separate words ("twenty-five", "so--").
    for part in lower.split(['-', '\u{2013}', '\u{2014}', '/']) {
        let clean: String = part
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '\'')
            .collect();
        let clean = clean.trim_matches('\'');
        if !clean.is_empty() {
            expand(clean, &mut out);
        }
    }
    out
}

fn expand(w: &str, out: &mut Vec<Token>) {
    let fixed: Option<&[&[&str]]> = match w {
        "can't" | "cannot" => Some(&[&["can"], &["not"]]),
        "won't" => Some(&[&["will"], &["not"]]),
        "shan't" => Some(&[&["shall"], &["not"]]),
        "ain't" => Some(&[&["am", "is", "are"], &["not"]]),
        "let's" => Some(&[&["let"], &["us"]]),
        "gonna" => Some(&[&["going"], &["to"]]),
        "wanna" => Some(&[&["want"], &["to"]]),
        "gotta" => Some(&[&["got"], &["to"]]),
        "kinda" => Some(&[&["kind"], &["of"]]),
        "sorta" => Some(&[&["sort"], &["of"]]),
        "dunno" => Some(&[&["do"], &["not"], &["know"]]),
        "lemme" => Some(&[&["let"], &["me"]]),
        "gimme" => Some(&[&["give"], &["me"]]),
        "y'all" => Some(&[&["you"], &["all"]]),
        "ok" | "okay" => Some(&[&["okay"]]),
        "'cause" | "cause" | "cos" | "cuz" => Some(&[&["because", "cause"]]),
        "em" => Some(&[&["them", "em"]]),
        _ => None,
    };
    let tok = |alts: &[&str]| -> Token { alts.iter().map(|s| s.to_string()).collect() };
    if let Some(toks) = fixed {
        out.extend(toks.iter().map(|t| tok(t)));
        return;
    }
    let suffixes: &[(&str, &[&str])] = &[
        ("n't", &["not"]),
        ("'re", &["are"]),
        ("'ve", &["have"]),
        ("'ll", &["will"]),
        ("'m", &["am"]),
        ("'d", &["would", "had"]),
    ];
    for (suffix, alts) in suffixes {
        if let Some(base) = w.strip_suffix(suffix).filter(|b| !b.is_empty()) {
            out.push(tok(&[base]));
            out.push(tok(alts));
            return;
        }
    }
    if let Some(base) = w.strip_suffix("'s").filter(|b| S_IS.contains(b)) {
        out.push(tok(&[base]));
        out.push(tok(&["is", "has"]));
        return;
    }
    out.push(tok(&[w]));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrong_ref(d: &Diff) -> Vec<&str> {
        d.reference
            .iter()
            .filter(|(_, ok)| !ok)
            .map(|(w, _)| w.as_str())
            .collect()
    }

    fn wrong_typed(d: &Diff) -> Vec<&str> {
        d.typed
            .iter()
            .filter(|(_, t)| *t != Typed::Ok)
            .map(|(w, _)| w.as_str())
            .collect()
    }

    fn classes(d: &Diff) -> Vec<Typed> {
        d.typed.iter().map(|(_, t)| *t).collect()
    }

    #[test]
    fn exact_match_ignoring_case_and_punctuation() {
        let d = compare("Well, I'm not sure!", "well i'm NOT sure");
        assert!(wrong_ref(&d).is_empty());
        assert!(wrong_typed(&d).is_empty());
        assert_eq!(d.score(), (4, 4));
    }

    #[test]
    fn contractions_are_equivalent_both_ways() {
        for (r, t) in [
            ("It's fine.", "it is fine"),
            ("it is fine", "it's fine"),
            ("I don't know.", "i do not know"),
            ("We can't go.", "we cannot go"),
            ("You'd better run.", "you had better run"),
            ("They're here", "they are here"),
            ("I'm gonna go", "I am going to go"),
            ("What\u{2019}s up?", "what's up"),
            ("Let's do it", "let us do it"),
        ] {
            let d = compare(r, t);
            assert!(wrong_ref(&d).is_empty(), "{r} vs {t}: {d:?}");
            assert!(wrong_typed(&d).is_empty(), "{r} vs {t}: {d:?}");
        }
    }

    #[test]
    fn marks_missed_and_wrong_words() {
        let d = compare("I could have told you that.", "I could told you this");
        assert_eq!(wrong_ref(&d), ["have", "that."]);
        assert_eq!(wrong_typed(&d), ["this"]);
        assert_eq!(d.typed[4].1, Typed::Wrong, "misheard, not extra");
        assert!(d.extra_words().is_empty());
        assert_eq!(d.score(), (4, 6));
    }

    #[test]
    fn marks_extra_words() {
        use Typed::*;
        // Inserted in the middle, at the start and at the end.
        let d = compare("I love you.", "well I really love you so much");
        assert_eq!(classes(&d), [Extra, Ok, Extra, Ok, Ok, Extra, Extra]);
        assert_eq!(d.extra_words(), ["well", "really", "so", "much"]);
        assert!(wrong_ref(&d).is_empty());
        assert_eq!(d.score(), (3, 3), "extra words do not change the score");

        // One misheard word plus one extra in the same stretch.
        let d = compare("I could have told you that.", "I could told you this too");
        assert_eq!(classes(&d), [Ok, Ok, Ok, Ok, Wrong, Extra]);
        assert_eq!(d.extra_words(), ["too"]);

        // Nothing in common: words stand in for the sentence's words first.
        let d = compare("Hi there", "hello over there friend");
        assert_eq!(classes(&d), [Wrong, Extra, Ok, Extra]);

        // A contraction where only part is in the sentence counts as extra.
        let d = compare("It works", "it's works");
        assert_eq!(classes(&d), [Extra, Ok]);
    }

    #[test]
    fn partially_heard_contraction_is_wrong() {
        // "I'd" -> "i would"; typing only "I" misses half of it.
        let d = compare("I'd love to.", "I love to");
        assert_eq!(wrong_ref(&d), ["I'd"]);
        assert!(wrong_typed(&d).is_empty());
    }

    #[test]
    fn possessive_is_not_expanded() {
        let d = compare("That's my friend's car.", "that is my friend's car");
        assert!(wrong_ref(&d).is_empty());
        let d = compare("my friend's car", "my friend is car");
        assert_eq!(wrong_ref(&d), ["friend's"]);
    }

    #[test]
    fn empty_input_and_dashes() {
        let d = compare("- Hi. - Hey!", "");
        assert_eq!(wrong_ref(&d), ["Hi.", "Hey!"]);
        assert_eq!(d.score(), (0, 2));
        let d = compare("twenty-five bucks", "twenty five bucks");
        assert!(wrong_ref(&d).is_empty());
    }
}
