/// A syntactic RFC 5646 language preference with lowercase ASCII spelling.
///
/// Normal, private-use, and grandfathered tags are accepted without checking registry
/// membership or locale support. Subtag order and deprecated identities are preserved;
/// this is not full RFC canonicalization. Support, negotiation, and fallback belong
/// to the locale subsystem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocaleTag(String);

/// Input-free facts about a rejected language preference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocaleTagError {
    Empty,
    TooLong { actual: usize },
    InvalidSyntax,
    DuplicateVariant,
    DuplicateExtension,
}

impl LocaleTag {
    /// Maximum input length in UTF-8 bytes; overlong input is never truncated.
    pub const MAX_BYTES: usize = 255;

    /// Parses the complete tag without trimming, replacing separators, or repairing Unicode.
    ///
    /// Empty input is rejected first, then the byte bound, then complete syntax. Only
    /// syntactically complete normal tags are checked for duplicates, with repeated
    /// variants preceding repeated extension singletons. Private-use repetitions are allowed.
    pub fn parse(input: &str) -> Result<Self, LocaleTagError> {
        if input.is_empty() {
            return Err(LocaleTagError::Empty);
        }
        if input.len() > Self::MAX_BYTES {
            return Err(LocaleTagError::TooLong {
                actual: input.len(),
            });
        }
        if !input
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(LocaleTagError::InvalidSyntax);
        }
        if !GRANDFATHERED
            .iter()
            .any(|tag| input.eq_ignore_ascii_case(tag))
        {
            parse_subtags(input)?;
        }
        Ok(Self(input.to_ascii_lowercase()))
    }

    /// Returns the accepted spelling, with only ASCII letter case normalized.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Retained locale allocation for owners enforcing a total memory bound.
    pub fn retained_heap_bytes(&self) -> usize {
        self.0.capacity()
    }
}

// RFC 5646 section 2.1 defines this fixed set of whole tags, including irregular forms.
const GRANDFATHERED: [&str; 26] = [
    "en-gb-oed",
    "i-ami",
    "i-bnn",
    "i-default",
    "i-enochian",
    "i-hak",
    "i-klingon",
    "i-lux",
    "i-mingo",
    "i-navajo",
    "i-pwn",
    "i-tao",
    "i-tay",
    "i-tsu",
    "sgn-be-fr",
    "sgn-be-nl",
    "sgn-ch-de",
    "art-lojban",
    "cel-gaulish",
    "no-bok",
    "no-nyn",
    "zh-guoyu",
    "zh-hakka",
    "zh-min",
    "zh-min-nan",
    "zh-xiang",
];

fn parse_subtags(input: &str) -> Result<(), LocaleTagError> {
    let mut subtags = input.split('-').peekable();
    let language = subtags.next().ok_or(LocaleTagError::InvalidSyntax)?;
    if language.eq_ignore_ascii_case("x") {
        return parse_private_use(subtags);
    }
    if !is_alpha(language, 2, 8) {
        return Err(LocaleTagError::InvalidSyntax);
    }
    if language.len() <= 3 {
        for _ in 0..3 {
            if subtags.peek().is_some_and(|tag| is_alpha(tag, 3, 3)) {
                subtags.next();
            } else {
                break;
            }
        }
    }
    if subtags.peek().is_some_and(|tag| is_alpha(tag, 4, 4)) {
        subtags.next();
    }
    if subtags.peek().is_some_and(|tag| {
        is_alpha(tag, 2, 2) || (tag.len() == 3 && tag.bytes().all(|byte| byte.is_ascii_digit()))
    }) {
        subtags.next();
    }

    let mut variants: Vec<&str> = Vec::new();
    let mut duplicate_variant = false;
    while let Some(variant) = subtags.next_if(|tag| is_variant(tag)) {
        duplicate_variant |= variants
            .iter()
            .any(|previous| variant.eq_ignore_ascii_case(previous));
        variants.push(variant);
    }
    let mut extensions: Vec<&str> = Vec::new();
    let mut duplicate_extension = false;
    while let Some(singleton) = subtags.next_if(|tag| {
        tag.len() == 1 && !tag.eq_ignore_ascii_case("x") && is_alphanumeric(tag, 1, 1)
    }) {
        duplicate_extension |= extensions
            .iter()
            .any(|previous| singleton.eq_ignore_ascii_case(previous));
        extensions.push(singleton);
        if subtags.next_if(|tag| is_alphanumeric(tag, 2, 8)).is_none() {
            return Err(LocaleTagError::InvalidSyntax);
        }
        while subtags.next_if(|tag| is_alphanumeric(tag, 2, 8)).is_some() {}
    }
    if subtags
        .next_if(|tag| tag.eq_ignore_ascii_case("x"))
        .is_some()
    {
        parse_private_use(subtags)?;
    } else if subtags.next().is_some() {
        return Err(LocaleTagError::InvalidSyntax);
    }

    // Defer duplicate diagnostics until every trailing subtag has passed the grammar.
    if duplicate_variant {
        Err(LocaleTagError::DuplicateVariant)
    } else if duplicate_extension {
        Err(LocaleTagError::DuplicateExtension)
    } else {
        Ok(())
    }
}

fn parse_private_use(
    mut subtags: std::iter::Peekable<std::str::Split<'_, char>>,
) -> Result<(), LocaleTagError> {
    let first = subtags.next().ok_or(LocaleTagError::InvalidSyntax)?;
    if is_alphanumeric(first, 1, 8) && subtags.all(|tag| is_alphanumeric(tag, 1, 8)) {
        Ok(())
    } else {
        Err(LocaleTagError::InvalidSyntax)
    }
}

fn is_alpha(tag: &str, minimum: usize, maximum: usize) -> bool {
    (minimum..=maximum).contains(&tag.len()) && tag.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_alphanumeric(tag: &str, minimum: usize, maximum: usize) -> bool {
    (minimum..=maximum).contains(&tag.len()) && tag.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn is_variant(tag: &str) -> bool {
    is_alphanumeric(tag, 5, 8)
        || (is_alphanumeric(tag, 4, 4)
            && tag.bytes().next().is_some_and(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::{LocaleTag, LocaleTagError};

    #[test]
    fn grammar_preserves_order_and_only_normalizes_ascii_case() {
        for (input, spelling) in [
            (
                "abc-def-ghi-jkl-Abcd-123-1abc-abcde-b-BB-a-AA-x-X",
                "abc-def-ghi-jkl-abcd-123-1abc-abcde-b-bb-a-aa-x-x",
            ),
            ("abcdefgh-Latn-US", "abcdefgh-latn-us"),
            ("IW", "iw"),
            ("i-KLINGON", "i-klingon"),
            ("X-A-a-12345678", "x-a-a-12345678"),
        ] {
            let tag = LocaleTag::parse(input).unwrap();
            assert_eq!(tag.as_str(), spelling);
            assert_eq!(tag, tag.clone());
            assert_eq!(LocaleTag::parse(spelling), Ok(tag));
        }
    }

    #[test]
    fn extension_singletons_cover_digits_and_letters_except_private_use_x() {
        for singleton in "0123456789abcdefghijklmnopqrstuvwyz".chars() {
            let input = format!("en-{singleton}-ab");
            assert_eq!(LocaleTag::parse(&input).unwrap().as_str(), input);
            let repeated = format!("en-{singleton}-ab-{}-cd", singleton.to_ascii_uppercase());
            assert_eq!(
                LocaleTag::parse(&repeated),
                Err(LocaleTagError::DuplicateExtension)
            );
        }
        assert!(LocaleTag::parse("en-x-a-a").is_ok());
    }

    #[test]
    fn malformed_suffixes_precede_duplicate_diagnostics() {
        for input in [
            "en-abcde-ABCDE-a",
            "en-a-aa-A",
            "en-abcde-ABCDE-a-aa-A-bb-x",
            "en-abcde-ABCDE-a-aa-A-bb-x-abcdefghi",
            "en-abcde-ABCDE-a-aa-A-bb--x-a",
        ] {
            assert_eq!(LocaleTag::parse(input), Err(LocaleTagError::InvalidSyntax));
        }
        assert_eq!(
            LocaleTag::parse("en-abcde-ABCDE-a-aa-A-bb"),
            Err(LocaleTagError::DuplicateVariant)
        );
        assert!(LocaleTag::parse("en-a-abcde-ABCDE-x-a-a").is_ok());
    }

    #[test]
    fn empty_and_utf8_byte_bounds_precede_syntax() {
        assert_eq!(LocaleTag::MAX_BYTES, 255);
        assert_eq!(LocaleTag::parse(""), Err(LocaleTagError::Empty));
        let maximum = format!("x{}-abcde", "-abcdefg".repeat(31));
        assert_eq!(maximum.len(), LocaleTag::MAX_BYTES);
        assert_eq!(LocaleTag::parse(&maximum).unwrap().as_str(), maximum);
        for input in [format!("{maximum}f"), " ".repeat(256), "é".repeat(128)] {
            assert_eq!(
                LocaleTag::parse(&input),
                Err(LocaleTagError::TooLong { actual: 256 })
            );
        }
        assert_eq!(
            LocaleTag::parse(&"é".repeat(127)),
            Err(LocaleTagError::InvalidSyntax)
        );
    }
}
