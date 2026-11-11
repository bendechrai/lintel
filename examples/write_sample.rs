//! Writes the sample house to a file: `cargo run --release --example write_sample -- out.json [name]`.

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("a path to write");
    let mut project = lintel::sample::house();
    if let Some(name) = args.next() {
        project.name = name;
    }
    lintel::io::save(&project, std::path::Path::new(&path)).expect("written");
}
