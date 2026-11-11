use rune::tok::{self, type_};
use rune::{AppSpec, Cx, El, col, text};
#[derive(Default)]
struct State;
fn view(_: &State, _: &mut Cx<State>) -> El {
    let t = tok::theme(true);
    col().w_full().h_full().center().fill(t.bg).child(text("Lintel").font(type_::TITLE).color(t.text))
}
fn main() {
    rune::run(AppSpec::new("Lintel", (320.0, 200.0), State::default, view));
}
