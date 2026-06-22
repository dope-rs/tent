pub(super) struct Text;

impl Text {
    pub(super) fn camel_to_dashed(name: &str) -> String {
        let mut out = String::with_capacity(name.len() * 2);
        for ch in name.chars() {
            if ch.is_uppercase() {
                out.push('-');
                out.extend(ch.to_lowercase());
            } else {
                out.push(ch);
            }
        }
        out
    }

    pub(super) fn unquote(literal: &str) -> String {
        Self::unescape(&literal[1..literal.len() - 1])
    }

    fn unescape(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars();
        while let Some(ch) = chars.next() {
            if ch != '\\' {
                out.push(ch);
                continue;
            }
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('0') => out.push('\0'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        }
        out
    }
}
