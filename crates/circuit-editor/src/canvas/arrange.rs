//! Aligning, spreading and snapping blocks to the grid.

use super::*;

/// How the Arrange menu lines selected blocks up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arrange {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
    SpreadAcross,
    SpreadDown,
}

impl Arrange {
    pub(super) const ALL: [Arrange; 8] = [Arrange::Left, Arrange::Center, Arrange::Right, Arrange::Top, Arrange::Middle, Arrange::Bottom, Arrange::SpreadAcross, Arrange::SpreadDown];

    pub(super) fn name(self) -> &'static str {
        match self {
            Arrange::Left => t!("Align left edges"),
            Arrange::Center => t!("Align centers (across)"),
            Arrange::Right => t!("Align right edges"),
            Arrange::Top => t!("Align top edges"),
            Arrange::Middle => t!("Align middles (down)"),
            Arrange::Bottom => t!("Align bottom edges"),
            Arrange::SpreadAcross => t!("Spread evenly across"),
            Arrange::SpreadDown => t!("Spread evenly down"),
        }
    }
}

/// Where each block of `rects` goes (its new top-left corner), lined up as `how` says.
pub fn arrange(rects: &[(String, Rect)], how: Arrange) -> Vec<(String, Pos2)> {
    let Some(bounds) = rects.iter().map(|(_, r)| *r).reduce(|a, b| a.union(b)) else { return Vec::new() };
    let place = |f: &dyn Fn(&Rect) -> Pos2| rects.iter().map(|(id, r)| (id.clone(), f(r))).collect();
    let spread = |across: bool| {
        let mut order: Vec<&(String, Rect)> = rects.iter().collect();
        order.sort_by(|a, b| if across { a.1.left().total_cmp(&b.1.left()) } else { a.1.top().total_cmp(&b.1.top()) });
        let size = |r: &Rect| if across { r.width() } else { r.height() };
        let span = if across { bounds.width() } else { bounds.height() };
        let total: f32 = order.iter().map(|(_, r)| size(r)).sum();
        // Equal gaps; never closer than a little room between them.
        let gap = if order.len() > 1 { ((span - total) / (order.len() - 1) as f32).max(GRID * 2.0) } else { 0.0 };
        let mut at = if across { bounds.left() } else { bounds.top() };
        order
            .into_iter()
            .map(|(id, r)| {
                let p = if across { pos2(at, r.top()) } else { pos2(r.left(), at) };
                at += size(r) + gap;
                (id.clone(), p)
            })
            .collect()
    };
    match how {
        Arrange::Left => place(&|r| pos2(bounds.left(), r.top())),
        Arrange::Center => place(&|r| pos2(bounds.center().x - r.width() / 2.0, r.top())),
        Arrange::Right => place(&|r| pos2(bounds.right() - r.width(), r.top())),
        Arrange::Top => place(&|r| pos2(r.left(), bounds.top())),
        Arrange::Middle => place(&|r| pos2(r.left(), bounds.center().y - r.height() / 2.0)),
        Arrange::Bottom => place(&|r| pos2(r.left(), bounds.bottom() - r.height())),
        Arrange::SpreadAcross => spread(true),
        Arrange::SpreadDown => spread(false),
    }
}

/// `v` to the nearest grid line.
pub(super) fn snap(v: f32) -> f32 {
    (v / GRID).round() * GRID
}
