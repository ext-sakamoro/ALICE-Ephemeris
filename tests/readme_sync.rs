//! README.md's first Rust example is the crate-level doctest of src/lib.rs
//! (README itself is not compiled), and its MSRV line names `rust-version`.

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn readme_example_is_the_doctest() {
    let readme = read("README.md");
    let start = readme
        .find("```rust\n")
        .expect("README has a ```rust block")
        + 8;
    let end = start + readme[start..].find("```\n").unwrap();
    let example: Vec<&str> = readme[start..end].lines().collect();

    let lib = read("src/lib.rs");
    let doc: Vec<String> = lib
        .lines()
        .filter_map(|l| l.strip_prefix("//!"))
        .map(|l| l.strip_prefix(' ').unwrap_or(l).to_owned())
        .collect();
    let open = doc
        .iter()
        .position(|l| l == "```")
        .expect("lib.rs has a doctest");
    let close = open + 1 + doc[open + 1..].iter().position(|l| l == "```").unwrap();
    let doctest = &doc[open + 1..close];
    assert!(!example.is_empty());
    assert_eq!(example, doctest);
}

#[test]
fn readme_msrv_is_rust_version() {
    let manifest = read("Cargo.toml");
    let v = manifest
        .lines()
        .find_map(|l| l.strip_prefix("rust-version = \""))
        .and_then(|l| l.strip_suffix('"'))
        .expect("Cargo.toml has rust-version");
    let readme = read("README.md");
    let line = readme
        .lines()
        .find(|l| l.contains("`rust-version`"))
        .expect("README names rust-version");
    assert!(line.contains(&format!("Rust {v} ")), "{line}");
}
