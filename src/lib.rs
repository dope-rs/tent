pub use tent_codegen::{
    css, css_body, html, html_body, load_css, load_css_body, load_html, load_html_body,
};

enum Segment {
    Static(&'static str),
    Owned(String),
}

pub struct Body {
    len: usize,
    segments: Vec<Segment>,
}

impl Body {
    pub fn with_capacity(parts: usize) -> Self {
        Self {
            len: 0,
            segments: Vec::with_capacity(parts),
        }
    }

    pub fn from_static(text: &'static str) -> Self {
        Self {
            len: text.len(),
            segments: vec![Segment::Static(text)],
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn finish(self) -> String {
        let mut out = String::with_capacity(self.len);
        for segment in self.segments {
            match segment {
                Segment::Static(text) => out.push_str(text),
                Segment::Owned(text) => out.push_str(&text),
            }
        }
        out
    }
}

pub trait Sink {
    fn put_static(&mut self, text: &'static str);
    fn put_owned(&mut self, text: &str);
}

impl Sink for String {
    fn put_static(&mut self, text: &'static str) {
        self.push_str(text);
    }

    fn put_owned(&mut self, text: &str) {
        self.push_str(text);
    }
}

impl Sink for Body {
    fn put_static(&mut self, text: &'static str) {
        self.len += text.len();
        self.segments.push(Segment::Static(text));
    }

    fn put_owned(&mut self, text: &str) {
        self.len += text.len();
        self.segments.push(Segment::Owned(text.to_owned()));
    }
}

pub trait Html {
    fn escaped<S: Sink>(&self, out: &mut S);
    fn raw<S: Sink>(&self, out: &mut S);
}

impl Html for str {
    fn escaped<S: Sink>(&self, out: &mut S) {
        let bytes = self.as_bytes();
        let mut start = 0;
        let mut idx = 0;
        while idx < bytes.len() {
            let replacement = match bytes[idx] {
                b'&' => "&amp;",
                b'<' => "&lt;",
                b'>' => "&gt;",
                b'"' => "&quot;",
                b'\'' => "&#x27;",
                _ => {
                    idx += 1;
                    continue;
                }
            };
            if start < idx {
                out.put_owned(&self[start..idx]);
            }
            out.put_static(replacement);
            idx += 1;
            start = idx;
        }
        if start < self.len() {
            out.put_owned(&self[start..]);
        }
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(self);
    }
}

impl Html for String {
    fn escaped<S: Sink>(&self, out: &mut S) {
        self.as_str().escaped(out);
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        self.as_str().raw(out);
    }
}

impl Html for std::borrow::Cow<'_, str> {
    fn escaped<S: Sink>(&self, out: &mut S) {
        self.as_ref().escaped(out);
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        self.as_ref().raw(out);
    }
}

impl<T: Html + ?Sized> Html for &T {
    fn escaped<S: Sink>(&self, out: &mut S) {
        (*self).escaped(out);
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        (*self).raw(out);
    }
}

impl Html for bool {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_static(if *self { "true" } else { "false" });
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_static(if *self { "true" } else { "false" });
    }
}

impl Html for u8 {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }
}

impl Html for u16 {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }
}

impl Html for u32 {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }
}

impl Html for u64 {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }
}

impl Html for usize {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }
}

impl Html for i8 {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }
}

impl Html for i16 {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }
}

impl Html for i32 {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }
}

impl Html for i64 {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }
}

impl Html for isize {
    fn escaped<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }

    fn raw<S: Sink>(&self, out: &mut S) {
        out.put_owned(&self.to_string());
    }
}
