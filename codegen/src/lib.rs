mod css;
mod html;
mod indent;
mod source;
mod text;

use proc_macro::TokenStream;
use proc_macro2::{self, Span};
use syn::parse::Parser;

use crate::css::Css;
use crate::html::Html;
use crate::indent::{Indent, IndentNode};

#[derive(Clone, Copy)]
enum Mode {
    Markup,
    Body,
}

#[derive(Clone, Copy)]
enum Origin {
    Inline,
    File,
}

trait Language {
    type Line: IndentNode<Output = Self::Tree>;
    type Tree;

    fn parse(input: &str) -> Result<Vec<Self::Line>, String>;
    fn emit_markup(roots: Vec<Self::Tree>) -> Result<proc_macro2::TokenStream, String>;
    fn emit_body(roots: Vec<Self::Tree>) -> Result<proc_macro2::TokenStream, String>;
}

struct Compiler;

impl Compiler {
    fn run<L: Language>(input: &str, mode: Mode) -> Result<proc_macro2::TokenStream, String> {
        let roots = Indent::try_build(L::parse(input)?)?;
        match mode {
            Mode::Markup => L::emit_markup(roots),
            Mode::Body => L::emit_body(roots),
        }
    }

    fn drive<L: Language>(input: TokenStream, origin: Origin, mode: Mode) -> TokenStream {
        let literal: syn::LitStr = match syn::parse(input) {
            Ok(literal) => literal,
            Err(error) => return error.to_compile_error().into(),
        };
        let span = literal.span();
        let source = match origin {
            Origin::Inline => literal.value(),
            Origin::File => {
                let dir = match std::env::var("CARGO_MANIFEST_DIR") {
                    Ok(dir) => dir,
                    Err(error) => {
                        return Self::error(
                            span,
                            format!("tent: CARGO_MANIFEST_DIR unset: {error:?}"),
                        );
                    }
                };
                let path = std::path::Path::new(&dir).join(literal.value());
                match std::fs::read_to_string(&path) {
                    Ok(text) => text,
                    Err(err) => return Self::error(span, format!("{}: {err}", path.display())),
                }
            }
        };
        match Self::run::<L>(&source, mode) {
            Ok(tokens) => tokens.into(),
            Err(message) => Self::error(span, message),
        }
    }

    fn error(span: Span, message: impl std::fmt::Display) -> TokenStream {
        syn::Error::new(span, message).to_compile_error().into()
    }
}

#[proc_macro]
pub fn html(input: TokenStream) -> TokenStream {
    Compiler::drive::<Html>(input, Origin::Inline, Mode::Markup)
}

#[proc_macro]
pub fn html_into(input: TokenStream) -> TokenStream {
    let parser = |input: syn::parse::ParseStream<'_>| {
        let literal = input.parse::<syn::LitStr>()?;
        input.parse::<syn::Token![,]>()?;
        let output = input.parse::<syn::Expr>()?;
        input.parse::<Option<syn::Token![,]>>()?;
        Ok((literal, output))
    };
    let (literal, output) = match parser.parse(input) {
        Ok(input) => input,
        Err(error) => return error.to_compile_error().into(),
    };
    let result = Html::parse(&literal.value())
        .and_then(Indent::try_build)
        .and_then(|roots| Html::emit_into(roots, quote::quote! { #output }));
    match result {
        Ok(tokens) => tokens.into(),
        Err(message) => Compiler::error(literal.span(), message),
    }
}

#[proc_macro]
pub fn html_body(input: TokenStream) -> TokenStream {
    Compiler::drive::<Html>(input, Origin::Inline, Mode::Body)
}

#[proc_macro]
pub fn load_html(input: TokenStream) -> TokenStream {
    Compiler::drive::<Html>(input, Origin::File, Mode::Markup)
}

#[proc_macro]
pub fn load_html_body(input: TokenStream) -> TokenStream {
    Compiler::drive::<Html>(input, Origin::File, Mode::Body)
}

#[proc_macro]
pub fn css(input: TokenStream) -> TokenStream {
    Compiler::drive::<Css>(input, Origin::Inline, Mode::Markup)
}

#[proc_macro]
pub fn css_body(input: TokenStream) -> TokenStream {
    Compiler::drive::<Css>(input, Origin::Inline, Mode::Body)
}

#[proc_macro]
pub fn load_css(input: TokenStream) -> TokenStream {
    Compiler::drive::<Css>(input, Origin::File, Mode::Markup)
}

#[proc_macro]
pub fn load_css_body(input: TokenStream) -> TokenStream {
    Compiler::drive::<Css>(input, Origin::File, Mode::Body)
}
