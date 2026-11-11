//! Prints a share link for a project file (or the sample house), for testing the browser build.
//! `cargo run --release --example share_link -- [project.json] [new name]`

fn main() {
    let mut args = std::env::args().skip(1);
    let mut project = match args.next() {
        Some(path) if path != "-" => lintel::io::load(std::path::Path::new(&path)).expect("a project file"),
        _ => lintel::sample::house(),
    };
    if let Some(name) = args.next() {
        project.name = name;
    }
    println!("{}", lintel::share::link(&project).expect("a link"));
}
