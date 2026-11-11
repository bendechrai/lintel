//! The furniture and appliance catalogue: what can be placed, its default size and finishes,
//! and the boxes each piece is made of.

use crate::model::{Finish, Furniture, Id, P2};

/// A catalogue entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub key: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    /// Width (across the front), depth, height.
    pub size: [f32; 3],
    pub lift: f32,
    pub finish: Finish,
    pub accent: Finish,
}

pub fn catalog() -> Vec<Piece> {
    Vec::new()
}

pub fn piece(key: &str) -> Option<Piece> {
    catalog().into_iter().find(|p| p.key == key)
}

/// A new piece of furniture of this kind at `at`, at its catalogue size and finishes.
pub fn place(key: &str, id: Id, at: P2) -> Option<Furniture> {
    let p = piece(key)?;
    Some(Furniture { id, kind: p.key.into(), name: p.name.into(), at, turn: 0.0, size: p.size, lift: p.lift, finish: p.finish, accent: p.accent })
}
