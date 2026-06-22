use proc_macro2::TokenStream;
use quote::quote;

use crate::Language;
use crate::indent::IndentNode;
use crate::source::{Source, Token};
use crate::text::Text;

pub(super) enum Item {
    Node { name: String, children: Vec<Self> },
    Declaration { property: String, value: String },
}

impl Item {
    fn join_name(namespace: &str, name: &str) -> String {
        if name.contains(',') {
            name.split(',')
                .map(|part| {
                    let part = part.trim();
                    match part.strip_prefix('&') {
                        Some(rest) => format!("{namespace}{rest}"),
                        None => format!("{namespace} {part}"),
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            match name.strip_prefix('&') {
                Some(rest) => format!("{namespace}{rest}"),
                None => format!("{namespace} {name}"),
            }
        }
    }

    fn flatten(self, namespace: Option<String>) -> (Option<Self>, Vec<Self>) {
        let Self::Node { name, children } = self else {
            return (Some(self), Vec::new());
        };

        if let Some(parent) = namespace.as_ref().filter(|_| name.starts_with("@media")) {
            let mut declarations = Vec::new();
            let mut nodes = Vec::new();
            for child in children {
                let (declaration, mut child_nodes) = child.flatten(Some(parent.clone()));
                if let Some(declaration) = declaration {
                    declarations.push(declaration);
                }
                nodes.append(&mut child_nodes);
            }
            if !declarations.is_empty() {
                nodes.push(Self::Node {
                    name: parent.clone(),
                    children: declarations,
                });
            }
            return (
                None,
                vec![Self::Node {
                    name,
                    children: nodes,
                }],
            );
        }

        let scoped = match &namespace {
            Some(namespace) => Self::join_name(namespace, &name),
            None => name,
        };
        let mut declarations = Vec::new();
        let mut nodes = Vec::new();
        for child in children {
            let (declaration, mut child_nodes) = child.flatten(Some(scoped.clone()));
            if let Some(declaration) = declaration {
                declarations.push(declaration);
            }
            nodes.append(&mut child_nodes);
        }
        nodes.push(Self::Node {
            name: scoped,
            children: declarations,
        });
        (None, nodes)
    }

    fn render(&self, out: &mut String) {
        match self {
            Self::Node { name, children } => {
                if children.is_empty() {
                    return;
                }
                out.push_str(name);
                out.push_str(" {");
                for child in children {
                    child.render(out);
                }
                out.push('}');
            }
            Self::Declaration { property, value } => {
                out.push_str(property);
                out.push_str(": ");
                out.push_str(&Text::unquote(value));
                out.push(';');
            }
        }
    }
}

pub(super) struct Node {
    level: usize,
    item: Item,
}

impl IndentNode for Node {
    type Output = Item;

    fn level(&self) -> usize {
        self.level
    }

    fn into_output(self) -> Item {
        self.item
    }

    fn adopt(&mut self, children: Vec<Item>) {
        match &mut self.item {
            Item::Node { children: slot, .. } => *slot = children,
            Item::Declaration { property, .. } => {
                panic!("CSS: declaration `{property}` cannot have children")
            }
        }
    }
}

enum State {
    StandBy,
    Prefix(char),
    Selector(String),
    Accumulated(String),
    Punctuated(String),
    DeclarationValue(String),
    Done(Item),
}

impl Node {
    fn parse(level: usize, tokens: Vec<Token>) -> Result<Self, String> {
        let mut state = State::StandBy;
        for token in tokens {
            state = match (state, token) {
                (State::StandBy, Token::Punct('@')) => State::Prefix('@'),
                (State::StandBy, Token::Ident(ident)) => State::Selector(ident),
                (State::StandBy, Token::Punct(p @ ('.' | '#' | '&'))) => {
                    State::Punctuated(p.to_string())
                }
                (State::Prefix(prefix), Token::Ident(ident)) => {
                    State::Selector(format!("{prefix}{}", Text::camel_to_dashed(&ident)))
                }
                (State::Selector(sel), Token::Punct('.')) => State::Punctuated(format!("{sel} .")),
                (State::Selector(sel), Token::Punct(',')) => State::Punctuated(format!("{sel}, ")),
                (State::Selector(sel), Token::Punct(':')) => State::DeclarationValue(sel),
                (State::DeclarationValue(property), Token::Literal(value)) => {
                    State::Done(Item::Declaration {
                        property: Text::camel_to_dashed(&property),
                        value,
                    })
                }
                (State::Selector(prev), Token::Literal(lit)) => {
                    State::Accumulated(format!("{prev} {}", Text::unquote(&lit)))
                }
                (State::Selector(prev), Token::Ident(ident)) => {
                    State::Accumulated(format!("{prev} {ident}"))
                }
                (State::Accumulated(prev), Token::Ident(ident)) => {
                    State::Accumulated(format!("{prev} {ident}"))
                }
                (State::Punctuated(prev), Token::Ident(ident)) => {
                    State::Accumulated(format!("{prev}{ident}"))
                }
                (State::Accumulated(prev), Token::Punct('-')) => {
                    State::Punctuated(format!("{prev}-"))
                }
                (State::Accumulated(prev), Token::Punct(',')) => {
                    State::Punctuated(format!("{prev}, "))
                }
                (State::Accumulated(prev), Token::Punct(p)) => {
                    State::Punctuated(format!("{prev} {p}"))
                }
                (State::Punctuated(prev), Token::Punct(p)) => {
                    State::Punctuated(format!("{prev}{p}"))
                }
                (State::Selector(prev), Token::Group { delimited, .. })
                | (State::Accumulated(prev), Token::Group { delimited, .. })
                | (State::Punctuated(prev), Token::Group { delimited, .. }) => {
                    State::Accumulated(format!("{prev}{delimited}"))
                }
                (_, token) => {
                    return Err(format!("CSS: unexpected `{}`", token.describe()));
                }
            };
        }

        let item = match state {
            State::Done(item) => item,
            State::Selector(name) | State::Accumulated(name) => Item::Node {
                name,
                children: Vec::new(),
            },
            _ => return Err("CSS: incomplete line".to_string()),
        };
        Ok(Self { level, item })
    }
}

pub(super) struct Css;

impl Css {
    fn render(roots: Vec<Item>) -> String {
        let mut flat = Vec::new();
        for item in roots {
            let (declaration, mut nodes) = item.flatten(None);
            debug_assert!(declaration.is_none(), "CSS: top-level declaration");
            flat.append(&mut nodes);
        }
        let mut css = String::new();
        for item in &flat {
            item.render(&mut css);
        }
        css
    }
}

impl Language for Css {
    type Line = Node;
    type Tree = Item;

    fn parse(input: &str) -> Result<Vec<Node>, String> {
        let mut vars: Vec<(String, String)> = Vec::new();
        let mut content: Vec<(usize, usize, String)> = Vec::new();
        for line in Source::lines(input) {
            let trimmed = line.text.trim();
            if let Some(rest) = trimmed.strip_prefix('$')
                && let Some(colon) = rest.find(':')
            {
                vars.push((
                    rest[..colon].trim().to_string(),
                    rest[colon + 1..].trim().to_string(),
                ));
            } else {
                content.push((line.num, line.level, line.text.to_string()));
            }
        }
        vars.sort_by_key(|v| std::cmp::Reverse(v.0.len()));

        let mut nodes = Vec::new();
        for (num, level, mut text) in content {
            for (name, value) in &vars {
                text = text.replace(&format!("${name}"), value);
            }
            let tokens = Source::tokenize(&text).map_err(|e| format!("CSS line {num}: {e}"))?;
            if tokens.is_empty() {
                continue;
            }
            nodes.push(Node::parse(level, tokens).map_err(|e| format!("CSS line {num}: {e}"))?);
        }
        Ok(nodes)
    }

    fn emit_markup(roots: Vec<Item>) -> Result<TokenStream, String> {
        let css = Self::render(roots);
        Ok(quote! { #css })
    }

    fn emit_body(roots: Vec<Item>) -> Result<TokenStream, String> {
        let css = Self::render(roots);
        Ok(quote! { ::tent::Body::from_static(#css) })
    }
}
