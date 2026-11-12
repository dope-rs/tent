use std::str::FromStr;

use proc_macro2::{Group, Literal, TokenStream, TokenTree};

pub(super) enum Token {
    Ident(String),
    Punct(char),
    Literal(Literal),
    Group(Group),
    RawGroup(TokenStream),
}

impl Token {
    fn from_tree(tree: TokenTree) -> Self {
        match tree {
            TokenTree::Ident(ident) => Self::Ident(ident.to_string()),
            TokenTree::Punct(punct) => Self::Punct(punct.as_char()),
            TokenTree::Literal(literal) => Self::Literal(literal),
            TokenTree::Group(group) => Self::Group(group),
        }
    }

    pub(super) fn describe(&self) -> String {
        match self {
            Self::Ident(text) => text.clone(),
            Self::Literal(literal) => literal.to_string(),
            Self::Group(group) => group.to_string(),
            Self::RawGroup(tokens) => tokens.to_string(),
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
        let stream = proc_macro::TokenStream::from_str(text)
            .map_err(|e| format!("failed to tokenize: {e}"))?;
        Ok(TokenStream::from(stream)
            .into_iter()
            .map(Token::from_tree)
            .collect())
    }
}
