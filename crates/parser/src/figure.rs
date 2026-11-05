//! Figure metadata shared by HTML and native renderers.

/// Read `title="..."` from a code fence's metadata (without its language).
/// Single quotes, unquoted words and escaped quotes/backslashes are supported.
/// Unknown attributes are skipped; empty or unfinished titles have no caption.
pub fn fence_title(meta: &str) -> Option<String> {
    let mut chars = meta.chars().peekable();
    while chars.peek().is_some() {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let mut key = String::new();
        while chars
            .peek()
            .is_some_and(|c| !c.is_whitespace() && *c != '=')
        {
            key.push(chars.next().unwrap());
        }
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        if chars.peek() != Some(&'=') {
            continue;
        }
        chars.next();
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let quote = chars.peek().copied().filter(|c| matches!(c, '\'' | '"'));
        if quote.is_some() {
            chars.next();
        }
        let mut value = String::new();
        let mut closed = quote.is_none();
        while let Some(c) = chars.next() {
            if Some(c) == quote {
                closed = true;
                break;
            }
            if quote.is_none() && c.is_whitespace() {
                break;
            }
            if c == '\\'
                && chars
                    .peek()
                    .is_some_and(|next| Some(*next) == quote || *next == '\\')
            {
                value.push(chars.next().unwrap());
            } else {
                value.push(c);
            }
        }
        if key == "title" {
            return (closed && !value.trim().is_empty())
                .then(|| value.trim().to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::fence_title;

    #[test]
    fn titles_are_explicit_and_preserve_unicode_and_punctuation() {
        for (meta, expected) in [
            ("title=流程图", Some("流程图")),
            ("title = '图 1 · 请求流程'", Some("图 1 · 请求流程")),
            (
                r#"layout="title=wrong" title="图 \"一\" <&>" other=true"#,
                Some("图 \"一\" <&>"),
            ),
            ("title=\"  \"", None),
            ("title=\"unfinished", None),
            ("untitled title", None),
            ("filename=example.mmd", None),
        ] {
            assert_eq!(fence_title(meta).as_deref(), expected, "{meta}");
        }
    }
}
