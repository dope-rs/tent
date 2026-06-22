use proc_macro2::TokenStream;
use quote::quote;

use crate::Language;
use crate::indent::IndentNode;
use crate::source::{Source, Token};
use crate::text::Text;

pub(super) enum Content {
    Fragment(Vec<Self>),
    Element {
        tag: String,
        classes: Vec<String>,
        properties: Vec<(String, String)>,
        contents: Vec<Self>,
    },
    Text(String),
    RawText(String),
    For {
        var: String,
        iter: String,
        contents: Vec<Self>,
    },
    If {
        condition: String,
        contents: Vec<Self>,
        else_branch: Option<Box<Self>>,
    },
    ElseIf {
        condition: String,
        contents: Vec<Self>,
    },
    Else {
        contents: Vec<Self>,
    },
}

impl Content {
    pub(super) fn to_markup(&self) -> TokenStream {
        if self.is_static() {
            let html = self.render_static();
            return quote! { ::std::borrow::Cow::<'static, str>::Borrowed(#html) };
        }
        let capacity = self.static_bytes();
        let stmts = self.statements();
        quote! {
            {
                use ::tent::{Html as _, Sink as _};
                let mut __tent = ::std::string::String::with_capacity(#capacity);
                #(#stmts)*
                ::std::borrow::Cow::<'static, str>::Owned(__tent)
            }
        }
    }

    pub(super) fn to_body(&self) -> TokenStream {
        if self.is_static() {
            let html = self.render_static();
            return quote! { ::tent::Body::from_static(#html) };
        }
        let parts = self.parts().max(1);
        let stmts = self.statements();
        quote! {
            {
                use ::tent::{Html as _, Sink as _};
                let mut __tent = ::tent::Body::with_capacity(#parts);
                #(#stmts)*
                __tent
            }
        }
    }

    fn statements(&self) -> Vec<TokenStream> {
        let mut accum = String::new();
        let mut stmts = Vec::new();
        self.collect(&mut accum, &mut stmts);
        Self::flush(&mut accum, &mut stmts);
        stmts
    }

    fn block(contents: &[Self]) -> Vec<TokenStream> {
        let mut accum = String::new();
        let mut stmts = Vec::new();
        for child in contents {
            child.collect(&mut accum, &mut stmts);
        }
        Self::flush(&mut accum, &mut stmts);
        stmts
    }

    fn collect(&self, accum: &mut String, stmts: &mut Vec<TokenStream>) {
        match self {
            Self::Fragment(children) => {
                for child in children {
                    child.collect(accum, stmts);
                }
            }
            Self::Element {
                tag,
                classes,
                properties,
                contents,
            } => {
                Self::open_tag(tag, classes, properties, accum, stmts);
                if !Self::is_void(tag) {
                    for child in contents {
                        child.collect(accum, stmts);
                    }
                    accum.push_str("</");
                    accum.push_str(tag);
                    accum.push('>');
                }
            }
            Self::Text(text) => match Self::literal(text) {
                Some(literal) => accum.push_str(&literal),
                None => {
                    Self::flush(accum, stmts);
                    let expr = Self::expr(text);
                    stmts.push(quote! { (#expr).escaped(&mut __tent); });
                }
            },
            Self::RawText(text) => {
                Self::flush(accum, stmts);
                let expr = Self::expr(text);
                stmts.push(quote! { (#expr).raw(&mut __tent); });
            }
            Self::For {
                var,
                iter,
                contents,
            } => {
                Self::flush(accum, stmts);
                let var = Self::expr(var);
                let iter = Self::expr(iter);
                let body = Self::block(contents);
                stmts.push(quote! { for #var in #iter { #(#body)* } });
            }
            Self::If {
                condition,
                contents,
                else_branch,
            } => {
                Self::flush(accum, stmts);
                let condition = Self::expr(condition);
                let body = Self::block(contents);
                match else_branch {
                    Some(branch) => {
                        let tail = branch.else_arm();
                        stmts.push(quote! { if #condition { #(#body)* } else #tail });
                    }
                    None => stmts.push(quote! { if #condition { #(#body)* } }),
                }
            }
            Self::ElseIf { .. } | Self::Else { .. } => {
                unreachable!("else branches are folded into If by merge_else")
            }
        }
    }

    fn else_arm(&self) -> TokenStream {
        match self {
            Self::If {
                condition,
                contents,
                else_branch,
            } => {
                let condition = Self::expr(condition);
                let body = Self::block(contents);
                match else_branch {
                    Some(branch) => {
                        let tail = branch.else_arm();
                        quote! { if #condition { #(#body)* } else #tail }
                    }
                    None => quote! { if #condition { #(#body)* } },
                }
            }
            Self::Else { contents } => {
                let body = Self::block(contents);
                quote! { { #(#body)* } }
            }
            _ => unreachable!("else branch is always If or Else"),
        }
    }

    fn open_tag(
        tag: &str,
        classes: &[String],
        properties: &[(String, String)],
        accum: &mut String,
        stmts: &mut Vec<TokenStream>,
    ) {
        accum.push('<');
        accum.push_str(tag);
        if !classes.is_empty() {
            accum.push_str(" class=\"");
            accum.push_str(&classes.join(" "));
            accum.push('"');
        }
        for (name, value) in properties {
            accum.push(' ');
            accum.push_str(name);
            if value.is_empty() {
                continue;
            }
            accum.push_str("=\"");
            match Self::literal(value) {
                Some(literal) => accum.push_str(&literal),
                None => {
                    Self::flush(accum, stmts);
                    let expr = Self::expr(value);
                    stmts.push(quote! { (#expr).escaped(&mut __tent); });
                }
            }
            accum.push('"');
        }
        accum.push('>');
    }

    fn flush(accum: &mut String, stmts: &mut Vec<TokenStream>) {
        if !accum.is_empty() {
            let text = std::mem::take(accum);
            stmts.push(quote! { __tent.put_static(#text); });
        }
    }

    fn literal(text: &str) -> Option<String> {
        text.starts_with('"').then(|| Text::unquote(text))
    }

    fn expr(text: &str) -> TokenStream {
        text.parse()
            .expect("tent: dynamic expression failed to tokenize")
    }

    fn is_void(tag: &str) -> bool {
        matches!(
            tag,
            "area"
                | "base"
                | "br"
                | "col"
                | "embed"
                | "hr"
                | "img"
                | "input"
                | "link"
                | "meta"
                | "param"
                | "source"
                | "track"
                | "wbr"
        )
    }

    fn is_static(&self) -> bool {
        match self {
            Self::Fragment(children) => children.iter().all(Self::is_static),
            Self::Element {
                properties,
                contents,
                ..
            } => {
                properties
                    .iter()
                    .all(|(_, value)| value.is_empty() || value.starts_with('"'))
                    && contents.iter().all(Self::is_static)
            }
            Self::Text(text) => text.starts_with('"'),
            Self::RawText(_)
            | Self::For { .. }
            | Self::If { .. }
            | Self::ElseIf { .. }
            | Self::Else { .. } => false,
        }
    }

    fn render_static(&self) -> String {
        let mut out = String::new();
        self.render_into(&mut out);
        out
    }

    fn render_into(&self, out: &mut String) {
        match self {
            Self::Fragment(children) => {
                for child in children {
                    child.render_into(out);
                }
            }
            Self::Element {
                tag,
                classes,
                properties,
                contents,
            } => {
                out.push('<');
                out.push_str(tag);
                if !classes.is_empty() {
                    out.push_str(" class=\"");
                    out.push_str(&classes.join(" "));
                    out.push('"');
                }
                for (name, value) in properties {
                    out.push(' ');
                    out.push_str(name);
                    if !value.is_empty() {
                        out.push_str("=\"");
                        out.push_str(&Text::unquote(value));
                        out.push('"');
                    }
                }
                out.push('>');
                if !Self::is_void(tag) {
                    for child in contents {
                        child.render_into(out);
                    }
                    out.push_str("</");
                    out.push_str(tag);
                    out.push('>');
                }
            }
            Self::Text(text) => out.push_str(&Text::unquote(text)),
            _ => unreachable!("render_static on a non-static node"),
        }
    }

    fn static_bytes(&self) -> usize {
        match self {
            Self::Fragment(children) => children.iter().map(Self::static_bytes).sum(),
            Self::Element {
                tag,
                classes,
                properties,
                contents,
            } => {
                let mut count = tag.len() + 2;
                if !classes.is_empty() {
                    count += 9 + classes.iter().map(String::len).sum::<usize>();
                    count += classes.len().saturating_sub(1);
                }
                for (name, value) in properties {
                    count += 1 + name.len();
                    if !value.is_empty() {
                        count += 3;
                        if value.starts_with('"') {
                            count += Text::unquote(value).len();
                        }
                    }
                }
                if !Self::is_void(tag) {
                    count += contents.iter().map(Self::static_bytes).sum::<usize>();
                    count += tag.len() + 3;
                }
                count
            }
            Self::Text(text) => {
                if text.starts_with('"') {
                    Text::unquote(text).len()
                } else {
                    0
                }
            }
            Self::RawText(_) | Self::ElseIf { .. } => 0,
            Self::For { contents, .. } | Self::Else { contents } => {
                contents.iter().map(Self::static_bytes).sum()
            }
            Self::If {
                contents,
                else_branch,
                ..
            } => {
                let then_bytes: usize = contents.iter().map(Self::static_bytes).sum();
                let else_bytes = else_branch.as_ref().map_or(0, |b| b.static_bytes());
                then_bytes.max(else_bytes)
            }
        }
    }

    fn parts(&self) -> usize {
        match self {
            Self::Fragment(children) => children.iter().map(Self::parts).sum(),
            Self::Element {
                properties,
                contents,
                ..
            } => {
                let dynamic = properties
                    .iter()
                    .filter(|(_, v)| !v.is_empty() && !v.starts_with('"'))
                    .count();
                2 + dynamic + contents.iter().map(Self::parts).sum::<usize>()
            }
            Self::Text(_) | Self::RawText(_) => 1,
            Self::ElseIf { .. } => 0,
            Self::For { contents, .. } | Self::Else { contents } => {
                contents.iter().map(Self::parts).sum()
            }
            Self::If {
                contents,
                else_branch,
                ..
            } => {
                let then_parts: usize = contents.iter().map(Self::parts).sum();
                let else_parts = else_branch.as_ref().map_or(0, |b| b.parts());
                then_parts.max(else_parts)
            }
        }
    }
}

pub(super) struct Node {
    level: usize,
    content: Content,
}

impl IndentNode for Node {
    type Output = Content;

    fn level(&self) -> usize {
        self.level
    }

    fn into_output(self) -> Content {
        self.content
    }

    fn adopt(&mut self, children: Vec<Content>) {
        let slot = match &mut self.content {
            Content::Element { contents, .. } if contents.is_empty() => contents,
            Content::For { contents, .. }
            | Content::If { contents, .. }
            | Content::ElseIf { contents, .. }
            | Content::Else { contents } => contents,
            _ => panic!("HTML: this line cannot hold a nested block"),
        };
        *slot = children;
    }

    fn finish_siblings(siblings: Vec<Content>) -> Vec<Content> {
        Self::merge_else(siblings)
    }
}

impl Node {
    fn merge_else(siblings: Vec<Content>) -> Vec<Content> {
        let mut out = Vec::with_capacity(siblings.len());
        let mut iter = siblings.into_iter().peekable();
        while let Some(item) = iter.next() {
            match item {
                Content::If {
                    condition,
                    contents,
                    ..
                } => {
                    let else_branch = Self::take_else(&mut iter);
                    out.push(Content::If {
                        condition,
                        contents,
                        else_branch,
                    });
                }
                other => out.push(other),
            }
        }
        out
    }

    fn take_else(
        iter: &mut std::iter::Peekable<std::vec::IntoIter<Content>>,
    ) -> Option<Box<Content>> {
        match iter.peek() {
            Some(Content::ElseIf { .. }) => {
                let Some(Content::ElseIf {
                    condition,
                    contents,
                }) = iter.next()
                else {
                    unreachable!()
                };
                let else_branch = Self::take_else(iter);
                Some(Box::new(Content::If {
                    condition,
                    contents,
                    else_branch,
                }))
            }
            Some(Content::Else { .. }) => iter.next().map(Box::new),
            _ => None,
        }
    }
}

enum State {
    StandBy,
    HasTag,
    PropertyName(String),
    PropertyValue(String),
    ClassName,
    Id,
    Done(Content),
}

pub(super) struct Html;

impl Html {
    fn attr_name(name: &str) -> String {
        if name == "viewBox" {
            name.to_string()
        } else {
            Text::camel_to_dashed(name)
        }
    }

    fn directive(num: usize, level: usize, trimmed: &str) -> Result<Option<Node>, String> {
        let content = if let Some(rest) = trimmed.strip_prefix("- for ") {
            let split = rest.find(" in ").ok_or_else(|| {
                format!("HTML line {num}: `- for VAR in EXPR` expected, got `{trimmed}`")
            })?;
            Content::For {
                var: rest[..split].trim().to_string(),
                iter: rest[split + 4..].trim().to_string(),
                contents: Vec::new(),
            }
        } else if let Some(rest) = trimmed.strip_prefix("- else if ") {
            Content::ElseIf {
                condition: rest.trim().to_string(),
                contents: Vec::new(),
            }
        } else if trimmed == "- else" {
            Content::Else {
                contents: Vec::new(),
            }
        } else if let Some(rest) = trimmed.strip_prefix("- if ") {
            Content::If {
                condition: rest.trim().to_string(),
                contents: Vec::new(),
                else_branch: None,
            }
        } else if let Some(rest) = trimmed.strip_prefix("| ") {
            let escaped = rest.replace('\\', "\\\\").replace('"', "\\\"");
            Content::Text(format!("\"{escaped}\""))
        } else {
            return Ok(None);
        };
        Ok(Some(Node { level, content }))
    }

    fn merge_raw(tokens: Vec<Token>) -> Vec<Token> {
        let mut out = Vec::with_capacity(tokens.len());
        let mut iter = tokens.into_iter().peekable();
        while let Some(token) = iter.next() {
            if matches!(token, Token::Punct('!'))
                && matches!(iter.peek(), Some(Token::Group { .. }))
            {
                let Some(Token::Group { inner, .. }) = iter.next() else {
                    unreachable!()
                };
                out.push(Token::RawGroup(inner));
            } else {
                out.push(token);
            }
        }
        out
    }

    fn parse_tag(tokens: Vec<Token>) -> Result<Content, String> {
        let mut state = State::StandBy;
        let mut tag: Option<String> = None;
        let mut classes: Vec<String> = Vec::new();
        let mut properties: Vec<(String, String)> = Vec::new();
        let mut contents: Vec<(String, bool)> = Vec::new();

        for token in tokens {
            state = match (state, token) {
                (State::StandBy, Token::Ident(ident)) => {
                    tag = Some(ident);
                    State::HasTag
                }
                (State::StandBy, Token::Literal(lit)) => State::Done(Content::Text(lit)),
                (State::StandBy, Token::Group { inner, .. }) => State::Done(Content::Text(inner)),
                (State::StandBy, Token::RawGroup(group)) => State::Done(Content::RawText(group)),
                (State::StandBy, Token::Punct('.')) => {
                    tag = Some("div".to_string());
                    State::ClassName
                }
                (State::StandBy, Token::Punct('#')) => {
                    tag = Some("div".to_string());
                    State::Id
                }
                (State::HasTag, Token::Punct('.')) => State::ClassName,
                (State::HasTag, Token::Punct('#')) => State::Id,
                (State::HasTag, Token::Ident(ident)) => {
                    State::PropertyName(Self::attr_name(&ident))
                }
                (State::PropertyName(name), Token::Punct('=')) => State::PropertyValue(name),
                (State::PropertyName(name), Token::Ident(ident)) => {
                    properties.push((name, String::new()));
                    State::PropertyName(Self::attr_name(&ident))
                }
                (State::PropertyName(name), Token::Punct('.')) => {
                    properties.push((name, String::new()));
                    State::ClassName
                }
                (State::PropertyName(name), Token::Punct('#')) => {
                    properties.push((name, String::new()));
                    State::Id
                }
                (State::PropertyName(name), Token::Literal(lit)) => {
                    properties.push((name, String::new()));
                    contents.push((lit, false));
                    State::HasTag
                }
                (State::PropertyName(name), Token::Group { inner, .. }) => {
                    properties.push((name, String::new()));
                    contents.push((inner, false));
                    State::HasTag
                }
                (State::PropertyName(name), Token::RawGroup(group)) => {
                    properties.push((name, String::new()));
                    contents.push((group, true));
                    State::HasTag
                }
                (State::PropertyValue(name), Token::Literal(lit)) => {
                    properties.push((name, lit));
                    State::HasTag
                }
                (State::PropertyValue(name), Token::Group { inner, .. }) => {
                    properties.push((name, inner));
                    State::HasTag
                }
                (State::HasTag, Token::Literal(lit)) => {
                    contents.push((lit, false));
                    State::HasTag
                }
                (State::HasTag, Token::Group { inner, .. }) => {
                    contents.push((inner, false));
                    State::HasTag
                }
                (State::HasTag, Token::RawGroup(group)) => {
                    contents.push((group, true));
                    State::HasTag
                }
                (State::ClassName, Token::Ident(ident)) => {
                    classes.push(ident);
                    State::HasTag
                }
                (State::Id, Token::Ident(ident)) => {
                    properties.push(("id".to_string(), format!("\"{ident}\"")));
                    State::HasTag
                }
                (_, token) => return Err(format!("HTML: unexpected `{}`", token.describe())),
            };
        }

        if let State::PropertyName(name) = state {
            properties.push((name, String::new()));
            state = State::HasTag;
        }
        if let State::Done(content) = state {
            return Ok(content);
        }
        let tag = tag.ok_or_else(|| "HTML: line has no tag".to_string())?;
        let contents = contents
            .into_iter()
            .map(|(text, raw)| {
                if raw {
                    Content::RawText(text)
                } else {
                    Content::Text(text)
                }
            })
            .collect();
        Ok(Content::Element {
            tag,
            classes,
            properties,
            contents,
        })
    }

    fn root(mut roots: Vec<Content>) -> Result<Content, String> {
        match roots.len() {
            0 => Err("HTML: empty template".to_string()),
            1 => Ok(roots.pop().unwrap()),
            _ => Ok(Content::Fragment(roots)),
        }
    }
}

impl Language for Html {
    type Line = Node;
    type Tree = Content;

    fn parse(input: &str) -> Result<Vec<Node>, String> {
        let mut nodes = Vec::new();
        for line in Source::lines(input) {
            let trimmed = line.text.trim();
            if let Some(node) = Self::directive(line.num, line.level, trimmed)? {
                nodes.push(node);
                continue;
            }
            let tokens =
                Source::tokenize(line.text).map_err(|e| format!("HTML line {}: {e}", line.num))?;
            let tokens = Self::merge_raw(tokens);
            if tokens.is_empty() {
                continue;
            }
            let content =
                Self::parse_tag(tokens).map_err(|e| format!("HTML line {}: {e}", line.num))?;
            nodes.push(Node {
                level: line.level,
                content,
            });
        }
        Ok(nodes)
    }

    fn emit_markup(roots: Vec<Content>) -> Result<TokenStream, String> {
        Ok(Self::root(roots)?.to_markup())
    }

    fn emit_body(roots: Vec<Content>) -> Result<TokenStream, String> {
        Ok(Self::root(roots)?.to_body())
    }
}
