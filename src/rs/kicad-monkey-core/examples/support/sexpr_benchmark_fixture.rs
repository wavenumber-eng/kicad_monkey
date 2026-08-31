pub const SPARSE_ITEMS: usize = 50_000;
pub const SPARSE_SELECT_EVERY: usize = 400;

pub fn speedy_shaped_sparse_fixture() -> (String, usize, usize) {
    let mut source = String::with_capacity(20 * 1024 * 1024);
    source.push_str("(kicad_sch\n");
    let mut selected = 0;
    for index in 0..SPARSE_ITEMS {
        source.push_str("(symbol (property \"Reference\" \"R");
        source.push_str(&index.to_string());
        source.push_str("\") (at 12.5 25.0) (effects (font (size 1.27 1.27)))");
        if index % SPARSE_SELECT_EVERY == 0 {
            source.push_str(" (target selected)");
            selected += 1;
        }
        source.push_str(")\n");
    }
    source.push_str(")\n");
    let visited = source.bytes().filter(|byte| *byte == b'(').count();
    (source, visited, selected)
}
