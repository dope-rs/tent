use std::str::FromStr;

use proc_macro::{TokenStream, TokenTree};

pub(super) enum Token {
    Ident(String),
    Punct(char),
    Literal(String),
    Group { delimited: String, inner: String },
    RawGroup(String),
}

impl Token {
    fn from_tree(tree: TokenTree) -> Self {
        match tree {
            TokenTree::Ident(ident) => Self::Ident(ident.to_string()),
            TokenTree::Punct(punct) => Self::Punct(punct.as_char()),
            TokenTree::Literal(literal) => Self::Literal(literal.to_string()),
            TokenTree::Group(group) => Self::Group {
                delimited: group.to_string(),
                inner: group.stream().to_string(),
            },
        }
    }

    pub(super) fn describe(&self) -> String {
        match self {
            Self::Ident(text) | Self::Literal(text) | Self::RawGroup(text) => text.clone(),
            Self::Group { delimited, .. } => delimited.clone(),
            Self::Punct(ch) => ch.to_string(),
        }
    }
}

pub(super) struct Line<'a> {
    pub(super) num: usize,
    pub(super) level: usize,
    pub(super) text: &'a str,
}

pub(super) struct Source;

impl Source {
    pub(super) fn lines(input: &str) -> impl Iterator<Item = Line<'_>> {
        input.lines().enumerate().filter_map(|(idx, raw)| {
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('/') {
                return None;
            }
            Some(Line {
                num: idx + 1,
                level: raw.find(|c: char| !c.is_whitespace()).unwrap_or(0),
                text: raw,
            })
        })
    }

    pub(super) fn tokenize(text: &str) -> Result<Vec<Token>, String> {
        let stream = TokenStream::from_str(text).map_err(|e| format!("failed to tokenize: {e}"))?;
        Ok(stream.into_iter().map(Token::from_tree).collect())
    }
}
