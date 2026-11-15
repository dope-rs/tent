mod common;

#[test]
fn html_static_nesting() {
    let out = tent::html!("div\n  span \"hello\"");
    assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
    assert_eq!(&*out, "<div><span>hello</span></div>");
}

#[test]
fn html_classes_and_nesting() {
    let out = tent::html!(".card\n  .title \"Hi\"");
    assert_eq!(
        &*out,
        "<div class=\"card\"><div class=\"title\">Hi</div></div>"
    );
}

#[test]
fn html_body_static_tracks_len() {
    let body = tent::html_body!("p \"text\"");
    assert_eq!(body.len(), "<p>text</p>".len());
    assert_eq!(body.finish(), "<p>text</p>");
}

#[test]
fn html_interpolates_expression() {
    let name = "world";
    let out = tent::html!("span (name)");
    assert_eq!(&*out, "<span>world</span>");
}

#[test]
fn html_body_for_loop_tracks_len() {
    let body = tent::html_body!("ul\n  - for n in -1i32..2\n    li (n)");
    let len = body.len();
    let rendered = body.finish();
    assert_eq!(rendered, "<ul><li>-1</li><li>0</li><li>1</li></ul>");
    assert_eq!(len, rendered.len());
}

#[test]
fn html_integer_boundaries() {
    common::integers(&[0, u8::MAX]);
    common::integers(&[0, u16::MAX]);
    common::integers(&[0, u32::MAX]);
    common::integers(&[0, u64::MAX]);
    common::integers(&[0, usize::MAX]);
    common::integers(&[i8::MIN, i8::MAX]);
    common::integers(&[i16::MIN, i16::MAX]);
    common::integers(&[i32::MIN, i32::MAX]);
    common::integers(&[i64::MIN, i64::MAX]);
    common::integers(&[isize::MIN, isize::MAX]);
}

#[test]
fn html_owned_output_reuses_capacity() {
    let message = String::from("é<&>\"'");
    let rows = [(i32::MIN, message.as_str()), (0, "plain")];
    let expected = "prefix<div data-id=\"-2147483648\">é&lt;&amp;&gt;&quot;&#x27;-2147483648</div><div data-id=\"0\">plain0</div>";
    let mut output = String::from("prefix");
    output
        .try_reserve_exact(expected.len() - output.len())
        .expect("output capacity");
    let pointer = output.as_ptr();
    let capacity = output.capacity();
    let mut calls = 0;
    let rendered = tent::html_into!(
        "- for (id, message) in &rows\n  div dataId=(id)\n    (message)\n    !{id}",
        {
            calls += 1;
            output
        },
    );
    drop(message);
    assert_eq!(rendered, expected);
    assert_eq!(rendered.as_ptr(), pointer);
    assert_eq!(rendered.capacity(), capacity);
    assert_eq!(calls, 1);
}

#[test]
fn html_output_binding_preserves_caller_names() {
    let __tent = String::from("<&");
    let output = tent::html_into!("p title=(__tent) (__tent)", String::new());
    assert_eq!(output, "<p title=\"&lt;&amp;\">&lt;&amp;</p>");
    assert_eq!(__tent, "<&");
    assert_eq!(tent::html!("p (__tent)"), "<p>&lt;&amp;</p>");
    assert_eq!(tent::html_body!("p (__tent)").finish(), "<p>&lt;&amp;</p>");
    let output = tent::html_into!(
        "- for __tent in [\"<\", \"&\"]\n  p (__tent)",
        String::new(),
    );
    assert_eq!(output, "<p>&lt;</p><p>&amp;</p>");
}

#[test]
fn html_static_output_appends_to_sink() {
    let output = tent::Body::from_static("prefix");
    let rendered = tent::html_into!("p \"text\"", output);
    assert_eq!(rendered.finish(), "prefix<p>text</p>");
}

#[test]
fn html_escapes_dynamic_text() {
    for (text, expected) in [
        ("", "<span></span>"),
        ("é漢🦀", "<span>é漢🦀</span>"),
        ("'&<>\"", "<span>&#x27;&amp;&lt;&gt;&quot;</span>"),
        (
            "é&漢<🦀>é\"漢'🦀",
            "<span>é&amp;漢&lt;🦀&gt;é&quot;漢&#x27;🦀</span>",
        ),
    ] {
        assert_eq!(tent::html!("span (text)"), expected);
        assert_eq!(tent::html_body!("span (text)").finish(), expected);
    }
}

#[test]
fn html_raw_skips_escaping() {
    let markup = "<b>x</b>";
    let out = tent::html!("span !{markup}");
    assert_eq!(&*out, "<span><b>x</b></span>");
}

#[test]
fn html_if_else_branches() {
    let body = tent::html_body!("div\n  - if 1 > 0\n    b \"yes\"\n  - else\n    b \"no\"");
    assert_eq!(body.finish(), "<div><b>yes</b></div>");
}

#[test]
fn html_else_if_preserves_following_branch() {
    let value = 1;
    let out = tent::html!(
        "div\n  - if value == 0\n    b \"zero\"\n  - else if value == 1\n    b \"one\"\n  - else\n    b \"other\"\n  - if value > 0\n    i \"positive\""
    );
    assert_eq!(out, "<div><b>one</b><i>positive</i></div>");
}

#[test]
fn html_loads_nested_template() {
    let out = tent::load_html!("tests/fixtures/basic.tent.html");
    assert_eq!(
        out,
        "<html><head><title>Test Page</title></head><body><h1>Hello!</h1><p>World</p></body></html>"
    );
}

#[test]
fn css_nests_selectors() {
    let css: &str = tent::css!(".box\n  color: \"red\"");
    assert_eq!(css, ".box {color: red;}");
}

#[test]
fn css_body_renders_static() {
    let body = tent::css_body!(".x\n  margin: \"0\"");
    assert_eq!(body.finish(), ".x {margin: 0;}");
}

#[test]
fn html_grouped_expressions_preserve_tokens() {
    let out = tent::html!(r#"p title=("<&".to_owned()) ("<&".to_owned())"#);
    assert_eq!(out, "<p title=\"&lt;&amp;\">&lt;&amp;</p>");
}

#[test]
fn html_grouped_literal_preserves_static_output() {
    let out = tent::html!(
        r#"p #value disabled empty="" title=() ("<&")
| \n "raw""#
    );
    assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
    assert_eq!(
        out,
        "<p id=\"value\" disabled empty=\"\" title><&</p>\\n \"raw\""
    );
}

#[test]
fn html_literal_tokens_preserve_dynamic_and_raw_output() {
    let out = tent::html!(r##"p r"<&" 42 !{"<b>"}"##);
    assert_eq!(out, "<p>&lt;&amp;42<b></p>");
}
