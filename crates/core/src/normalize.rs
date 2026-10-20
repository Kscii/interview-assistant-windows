use unicode_normalization::UnicodeNormalization;

/// Deliberately conservative: do not collapse digits, negations, apostrophes,
/// decimal points, hyphens, or technical operators (C++, C#, .NET, !=).
pub fn question_key(text: &str) -> String {
    let chars: Vec<char> = text.nfkc().flat_map(char::to_lowercase).collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).and_then(|p| chars.get(p)).copied();
        let next = chars.get(i + 1).copied();
        let numeric_comma = c == ','
            && prev.is_some_and(|x| x.is_ascii_digit())
            && next.is_some_and(|x| x.is_ascii_digit())
            && {
                let start = chars[..i]
                    .iter()
                    .rposition(|c| !c.is_ascii_digit() && *c != ',')
                    .map_or(0, |p| p + 1);
                let end = chars[i..]
                    .iter()
                    .position(|c| !c.is_ascii_digit() && *c != ',')
                    .map_or(chars.len(), |p| i + p);
                let number: String = chars[start..end].iter().collect();
                let mut groups = number.trim_end_matches(',').split(',');
                let first = groups.next().unwrap_or("");
                (1..=3).contains(&first.len()) && groups.all(|g| g.len() == 3)
            };
        if numeric_comma {
            continue;
        }
        let punctuation = matches!(
            c,
            '?' | '？' | '！' | '，' | ',' | ':' | '：' | ';' | '；' | '"' | '“' | '”'
        ) || (c == '!' && next != Some('='))
            || (c == '.' && !next.is_some_and(|x| x.is_alphanumeric()));
        out.push(if punctuation || c.is_whitespace() {
            ' '
        } else if c == '’' {
            '\''
        } else {
            c
        });
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn punctuation_and_case_match() {
        assert_eq!(
            question_key("  WHY do you like AI? "),
            question_key("why do you like AI！")
        );
        assert_eq!(question_key("10,000 years"), question_key("10000 years"));
    }
    #[test]
    fn meaning_is_not_erased() {
        for (a, b) in [
            ("can", "can't"),
            ("C", "C++"),
            ("C#", "C"),
            ("1.5", "15"),
            (".NET", "NET"),
            ("a != b", "a = b"),
            ("past 100", "past 1000"),
            ("choose 1,2,3", "choose 123"),
            ("value 1,5", "value 15"),
        ] {
            assert_ne!(question_key(a), question_key(b), "{a} / {b}");
        }
    }
}
