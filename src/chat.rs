//! The measuring assistant's panel: the conversation, a box to type or dictate into, and the
//! place to give it your own API key.

use rune::{El, ScrollSpec, Tx, col, row, spacer, text, text_input};

use crate::app::State;
use crate::widgets::*;

const DICTATE: &str = if cfg!(target_os = "macos") {
    "To speak instead of typing, click in the box and start the Mac's dictation (press the microphone key, or Fn twice)."
} else if cfg!(target_os = "windows") {
    "To speak instead of typing, click in the box and press Windows + H for voice typing."
} else {
    "To speak instead of typing, click in the box and use your desktop's dictation."
};

fn bubble(th: &Th, i: usize, line: &crate::assistant::Line) -> El {
    let fill = if line.from_you { th.accent.alpha(if th.dark { 0.30 } else { 0.14 }) } else { th.field };
    let b = col().key(format!("chat-line-{i}")).max_w(236.0).px(9.0).py(6.0).radius(8.0).fill(fill).border(1.0, th.border).child(text(line.text.clone()).font(LABEL).color(th.text).wrap());
    if line.from_you { row().w_full().child(spacer()).child(b) } else { row().w_full().child(b) }
}

fn field(th: &Th, inner: El) -> El {
    row().grow(1.0).min_w(0.0).h(28.0).px(8.0).radius(6.0).fill(th.field).border(1.0, th.border).center_y().child(inner)
}

pub fn view(s: &State, th: &Th) -> El {
    let a = &s.assistant;
    let head = row()
        .w_full()
        .center_y()
        .gap(6.0)
        .child(text("MEASURING ASSISTANT").font(HEAD).color(th.dim))
        .child(spacer())
        .child(ghost(th, "chat-forget", "Start over", false).on_press(|s: &mut State| s.forget_conversation()))
        .child(ghost(th, "chat-close", "Close", false).on_press(|s: &mut State| s.assistant.open = false));

    let mut talk: Vec<El> = Vec::new();
    if a.lines.is_empty() {
        talk.push(
            text("Read your measurements out and the assistant draws the house: \"The living room is 5.2 by 4 metres. There's a door 820 wide, 1.2 metres from the left corner of the south wall.\" It asks for one thing at a time, and everything it does can be undone.")
                .font(LABEL)
                .color(th.dim)
                .wrap(),
        );
        talk.push(text(DICTATE).font(SMALL).color(th.dim).wrap().mt(6.0));
    }
    talk.extend(a.lines.iter().enumerate().map(|(i, l)| bubble(th, i, l)));
    if !a.status.is_empty() {
        talk.push(text(a.status.clone()).key("chat-status").font(SMALL).color(th.dim).wrap());
    }
    let talk = col().key("chat-scroll").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: true }).child(col().w_full().gap(6.0).children(talk));

    let mut foot: Vec<El> = Vec::new();
    if a.api_key().is_none() || !a.key.text.is_empty() {
        foot.push(field(th, text_input(&a.key, VALUE).single_line().key("chat-key").grow(1.0).min_w(0.0).color(th.text).placeholder("Your Anthropic API key", th.dim).caret(th.text, th.accent.alpha(0.3)).on_edit(|s: &mut State| &mut s.assistant.key)));
        foot.push(
            text("The assistant is Claude, on your own key from console.anthropic.com. The key is held for this session only: it is never saved and never goes into a project or a shared link. Or start Lintel with ANTHROPIC_API_KEY set. What you say here, and a description of the walls and rooms, is sent to api.anthropic.com.")
                .font(SMALL)
                .color(th.dim)
                .wrap(),
        );
    }
    foot.push(
        row()
            .w_full()
            .gap(6.0)
            .child(field(
                th,
                text_input(&a.input, VALUE).single_line().key("chat-input").grow(1.0).min_w(0.0).color(th.text).placeholder("Say a measurement…", th.dim).caret(th.text, th.accent.alpha(0.3)).on_edit(|s: &mut State| &mut s.assistant.input).on_submit(|s: &mut State, _: &mut Tx| s.ask()),
            ))
            .child(button(th, "chat-send", if a.busy() { "…" } else { "Send" }, true).on_press(|s: &mut State| s.ask())),
    );
    foot.push(toggle(th, "Speak replies", "chat-speak", a.speak, |s: &mut State, on| s.assistant.speak = on));

    col()
        .key("assistant")
        .w(304.0)
        .h_full()
        .min_h(0.0)
        .padding(12.0)
        .gap(8.0)
        .fill(th.panel)
        .border_sides([0.0, 0.0, 0.0, 1.0], th.border)
        .child(head)
        .child(talk)
        .child(col().w_full().gap(6.0).children(foot))
}
