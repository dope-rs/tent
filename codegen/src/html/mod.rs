use proc_macro2::{Literal, Span, TokenStream, TokenTree};
use quote::{quote, quote_spanned};

use crate::Language;
use crate::indent::IndentNode;
use crate::source::{Source, Token};
use crate::text::Text;

pub(super) enum Value {
    Static(String),
    Dynamic(TokenStream),
}

impl Value {
    fn from_literal(literal: Literal) -> Self {
        if let Some(text) = Text::unquote(&literal) {
            Self::Static(text)
        } else {
            Self::Dynamic(TokenTree::Literal(literal).into())
        }
    }

    fn from_tokens(tokens: TokenStream) -> Self {
        let mut iter = tokens.clone().into_iter();
        if let Some(TokenTree::Literal(literal)) = iter.next()
            && iter.next().is_none()
            && let Some(text) = Text::unquote(&literal)
        {
            Self::Static(text)
        } else {
            Self::Dynamic(tokens)
        }
    }

    fn literal(&self) -> Option<&str> {
        match self {
            Self::Static(text) => Some(text),
            Self::Dynamic(_) => None,
        }
    }

    fn collect(&self, accum: &mut String, stmts: &mut Vec<TokenStream>) {
        match self {
            Self::Static(text) => accum.push_str(text),
            Self::Dynamic(expr) => {
                Content::flush(accum, stmts);
                stmts.push(quote_spanned! {Span::mixed_site()=> (#expr).escaped(&mut __tent); });
            }
        }
    }
}

pub(super) enum Content {
    Fragment(Vec<Self>),
    Element {
        tag: String,
        classes: Vec<String>,
        properties: Vec<(String, Option<Value>)>,
        contents: Vec<Self>,
    },
    Text(Value),
    RawText(TokenStream),
    For {
        var: TokenStream,
        iter: TokenStream,
        contents: Vec<Self>,
    },
    If {
        condition: TokenStream,
        contents: Vec<Self>,
        else_branch: Option<Box<Self>>,
    },
    ElseIf {
        condition: TokenStream,
        contents: Vec<Self>,
    },
    Else {
        contents: Vec<Self>,
    },
}

impl Content {
    pub(super) fn to_markup(&self) -> TokenStream {
        let (html, stmts) = self.statements();
        if self.is_static() {
            return quote! { ::std::borrow::Cow::<'static, str>::Borrowed(#html) };
        }
        let capacity = self.static_bytes();
        let output = Self::emit(
            quote! { ::std::string::String },
            quote! { ::std::string::String::with_capacity(#capacity) },
            html,
            stmts,
        );
        quote! { ::std::borrow::Cow::<'static, str>::Owned(#output) }
    }

    fn emit(
        ty: TokenStream,
        output: TokenStream,
        mut accum: String,
        mut stmts: Vec<TokenStream>,
    ) -> TokenStream {
        Self::flush(&mut accum, &mut stmts);
        quote_spanned! {Span::mixed_site()=>
            {
                use ::tent::{Html as _, Sink as _};
                let mut __tent: #ty = #output;
                #(#stmts)*
                __tent
            }
        }
    }

    pub(super) fn to_body(&self) -> TokenStream {
        let (html, stmts) = self.statements();
        if self.is_static() {
            return quote! { ::tent::Body::from_static(#html) };
        }
        let parts = self.parts().max(1);
        Self::emit(
            quote! { ::tent::Body },
            quote! { ::tent::Body::with_capacity(#parts) },
            html,
            stmts,
        )
    }

    fn statements(&self) -> (String, Vec<TokenStream>) {
        let mut accum = String::new();
        let mut stmts = Vec::new();
        self.collect(&mut accum, &mut stmts);
        (accum, stmts)
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
            Self::Text(value) => value.collect(accum, stmts),
            Self::RawText(expr) => {
                Self::flush(accum, stmts);
                stmts.push(quote_spanned! {Span::mixed_site()=> (#expr).raw(&mut __tent); });
            }
            Self::For {
                var,
                iter,
                contents,
            } => {
                Self::flush(accum, stmts);
                let body = Self::block(contents);
                stmts.push(quote! { for #var in #iter { #(#body)* } });
            }
            Self::If {
                condition,
                contents,
                else_branch,
            } => {
                Self::flush(accum, stmts);
                stmts.push(Self::if_branch(condition, contents, else_branch.as_deref()));
            }
            Self::ElseIf { .. } | Self::Else { .. } => {
                unreachable!("branch")
            }
        }
    }

    fn if_branch(
        condition: &TokenStream,
        contents: &[Self],
        else_branch: Option<&Self>,
    ) -> TokenStream {
        let body = Self::block(contents);
        match else_branch {
            Some(branch) => {
                let tail = branch.else_arm();
                quote! { if #condition { #(#body)* } else #tail }
            }
            None => quote! { if #condition { #(#body)* } },
        }
    }

    fn else_arm(&self) -> TokenStream {
        match self {
            Self::If {
                condition,
                contents,
                else_branch,
            } => Self::if_branch(condition, contents, else_branch.as_deref()),
            Self::Else { contents } => {
                let body = Self::block(contents);
                quote! { { #(#body)* } }
            }
            _ => unreachable!("branch"),
        }
    }

    fn open_tag(
        tag: &str,
        classes: &[String],
        properties: &[(String, Option<Value>)],
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
            let Some(value) = value else {
                continue;
            };
            accum.push_str("=\"");
            value.collect(accum, stmts);
            accum.push('"');
        }
        accum.push('>');
    }

    fn flush(accum: &mut String, stmts: &mut Vec<TokenStream>) {
        if !accum.is_empty() {
            let text = std::mem::take(accum);
            stmts.push(quote_spanned! {Span::mixed_site()=> __tent.put_static(#text); });
        }
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
                    .all(|(_, value)| value.as_ref().is_none_or(|v| v.literal().is_some()))
                    && contents.iter().all(Self::is_static)
            }
            Self::Text(value) => value.literal().is_some(),
            Self::RawText(_)
            | Self::For { .. }
            | Self::If { .. }
            | Self::ElseIf { .. }
            | Self::Else { .. } => false,
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
                    if let Some(value) = value {
                        count += 3 + value.literal().map_or(0, str::len);
                    }
                }
                if !Self::is_void(tag) {
                    count += contents.iter().map(Self::static_bytes).sum::<usize>();
                    count += tag.len() + 3;
                }
                count
            }
            Self::Text(value) => value.literal().map_or(0, str::len),
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
                    .filter(|(_, v)| matches!(v, Some(Value::Dynamic(_))))
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

    fn try_adopt(&mut self, children: Vec<Content>) -> Result<(), String> {
        let slot = match &mut self.content {
            Content::Element { contents, .. } if contents.is_empty() => contents,
            Content::For { contents, .. }
            | Content::If { contents, .. }
            | Content::ElseIf { contents, .. }
            | Content::Else { contents } => contents,
            _ => return Err("HTML: this line cannot hold a nested block".to_string()),
        };
        *slot = children;
        Ok(())
    }

    fn try_finish_siblings(siblings: Vec<Content>) -> Result<Vec<Content>, String> {
        Self::try_merge_else(siblings)
    }
}

impl Node {
    fn try_merge_else(siblings: Vec<Content>) -> Result<Vec<Content>, String> {
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
                Content::ElseIf { .. } | Content::Else { .. } => {
                    return Err("HTML: else without a preceding if".to_string());
                }
                other => out.push(other),
            }
        }
        Ok(out)
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
                    unreachable!("branch")
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
    pub(super) fn emit_into(
        roots: Vec<Content>,
        output: TokenStream,
    ) -> Result<TokenStream, String> {
        let (html, stmts) = Self::root(roots)?.statements();
        Ok(Content::emit(
            quote! { ::std::string::String },
            output,
            html,
            stmts,
        ))
    }

    fn attr_name(name: &str) -> String {
        if name == "viewBox" {
            name.to_string()
        } else {
            Text::camel_to_dashed(name)
        }
    }

    fn directive(num: usize, level: usize, trimmed: &str) -> Result<Option<Node>, String> {
        let expression = |text: &str| {
            text.trim()
                .parse::<TokenStream>()
                .map_err(|error| format!("HTML line {num}: failed to tokenize: {error}"))
        };
        let content = if let Some(rest) = trimmed.strip_prefix("- for ") {
            let split = rest.find(" in ").ok_or_else(|| {
                format!("HTML line {num}: `- for VAR in EXPR` expected, got `{trimmed}`")
            })?;
            Content::For {
                var: expression(&rest[..split])?,
                iter: expression(&rest[split + 4..])?,
                contents: Vec::new(),
            }
        } else if let Some(rest) = trimmed.strip_prefix("- else if ") {
            Content::ElseIf {
                condition: expression(rest)?,
                contents: Vec::new(),
            }
        } else if trimmed == "- else" {
            Content::Else {
                contents: Vec::new(),
            }
        } else if let Some(rest) = trimmed.strip_prefix("- if ") {
            Content::If {
                condition: expression(rest)?,
                contents: Vec::new(),
                else_branch: None,
            }
        } else if let Some(rest) = trimmed.strip_prefix("| ") {
            Content::Text(Value::Static(rest.to_string()))
        } else {
            return Ok(None);
        };
        Ok(Some(Node { level, content }))
    }

    fn merge_raw(tokens: Vec<Token>) -> Vec<Token> {
        let mut out = Vec::with_capacity(tokens.len());
        let mut iter = tokens.into_iter().peekable();
        while let Some(token) = iter.next() {
            if matches!(token, Token::Punct('!')) && matches!(iter.peek(), Some(Token::Group(_))) {
                let Some(Token::Group(group)) = iter.next() else {
                    unreachable!("group")
                };
                out.push(Token::RawGroup(group.stream()));
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
        let mut properties: Vec<(String, Option<Value>)> = Vec::new();
        let mut contents: Vec<Content> = Vec::new();

        for token in tokens {
            state = match (state, token) {
                (State::StandBy, Token::Ident(ident)) => {
                    tag = Some(ident);
                    State::HasTag
                }
                (State::StandBy, Token::Literal(lit)) => {
                    State::Done(Content::Text(Value::from_literal(lit)))
                }
                (State::StandBy, Token::Group(group)) => {
                    State::Done(Content::Text(Value::from_tokens(group.stream())))
                }
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
                    properties.push((name, None));
                    State::PropertyName(Self::attr_name(&ident))
                }
                (State::PropertyName(name), Token::Punct('.')) => {
                    properties.push((name, None));
                    State::ClassName
                }
                (State::PropertyName(name), Token::Punct('#')) => {
                    properties.push((name, None));
                    State::Id
                }
                (State::PropertyName(name), Token::Literal(lit)) => {
                    properties.push((name, None));
                    contents.push(Content::Text(Value::from_literal(lit)));
                    State::HasTag
                }
                (State::PropertyName(name), Token::Group(group)) => {
                    properties.push((name, None));
                    contents.push(Content::Text(Value::from_tokens(group.stream())));
                    State::HasTag
                }
                (State::PropertyName(name), Token::RawGroup(group)) => {
                    properties.push((name, None));
                    contents.push(Content::RawText(group));
                    State::HasTag
                }
                (State::PropertyValue(name), Token::Literal(lit)) => {
                    properties.push((name, Some(Value::from_literal(lit))));
                    State::HasTag
                }
                (State::PropertyValue(name), Token::Group(group)) => {
                    let inner = group.stream();
                    let value = (!inner.is_empty()).then(|| Value::from_tokens(inner));
                    properties.push((name, value));
                    State::HasTag
                }
                (State::HasTag, Token::Literal(lit)) => {
                    contents.push(Content::Text(Value::from_literal(lit)));
                    State::HasTag
                }
                (State::HasTag, Token::Group(group)) => {
                    contents.push(Content::Text(Value::from_tokens(group.stream())));
                    State::HasTag
                }
                (State::HasTag, Token::RawGroup(group)) => {
                    contents.push(Content::RawText(group));
                    State::HasTag
                }
                (State::ClassName, Token::Ident(ident)) => {
                    classes.push(ident);
                    State::HasTag
                }
                (State::Id, Token::Ident(ident)) => {
                    properties.push(("id".to_string(), Some(Value::Static(ident))));
                    State::HasTag
                }
                (_, token) => return Err(format!("HTML: unexpected `{}`", token.describe())),
            };
        }

        if let State::PropertyName(name) = state {
            properties.push((name, None));
            state = State::HasTag;
        }
        if let State::Done(content) = state {
            return Ok(content);
        }
        let tag = tag.ok_or_else(|| "HTML: line has no tag".to_string())?;
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
