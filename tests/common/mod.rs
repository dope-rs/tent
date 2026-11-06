pub(super) fn integers<T: tent::Html + std::fmt::Display>(values: &[T]) {
    for value in values {
        let value = std::hint::black_box(value);
        let out = tent::html!("(value)\n\"/\"\n!{value}");
        assert_eq!(out, format!("{value}/{value}"));
    }
}
