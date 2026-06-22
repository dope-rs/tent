#[test]
fn html_static_nesting() {
    let out = tent::html!("div\n  span \"hello\"");
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
    let body = tent::html_body!("ul\n  - for n in 0u32..3\n    li (n)");
    let len = body.len();
    let rendered = body.finish();
    assert_eq!(rendered, "<ul><li>0</li><li>1</li><li>2</li></ul>");
    assert_eq!(len, rendered.len());
}

#[test]
fn html_escapes_dynamic_text() {
    let danger = "<b>";
    let out = tent::html!("span (danger)");
    assert_eq!(&*out, "<span>&lt;b&gt;</span>");
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
fn css_nests_selectors() {
    let css: &str = tent::css!(".box\n  color: \"red\"");
    assert_eq!(css, ".box {color: red;}");
}

#[test]
fn css_body_renders_static() {
    let body = tent::css_body!(".x\n  margin: \"0\"");
    assert_eq!(body.finish(), ".x {margin: 0;}");
}
