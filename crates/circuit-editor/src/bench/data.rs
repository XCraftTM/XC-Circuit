//! Made-up workspaces for the benchmarks (`benches.rs` and `--fps`): blocks as
//! in a real bot, wired up.

use circuit_core::{Link, Node, Workspace};
use serde_json::{Value, json};

/// Space between blocks.
pub const STEP: [f32; 2] = [380.0, 340.0];

/// A workspace of `n` blocks in rows of `cols`: Send Message, Math, If and
/// Log in turn, each joined to the next by a green wire, and a value wire
/// from every Math into the block after it.
pub fn workspace(name: &str, n: usize, cols: usize) -> Workspace {
    let mut ws = Workspace { name: name.into(), ..Default::default() };
    for i in 0..n {
        let (block, values, pins): (&str, Value, Vec<String>) = match i % 4 {
            0 => ("discord.send_message@3", json!({ "text": format!("Hello ${{user.username}}, this is message {i}") }), vec![]),
            1 => ("core.math@1", json!({ "a": i, "op": "multiply", "b": 2 }), vec![]),
            2 => ("core.if@1", json!({}), vec![]),
            _ => ("core.log@1", json!({ "message": format!("Step {i} done") }), vec!["message".into()]),
        };
        let values = values.as_object().cloned().unwrap_or_default();
        let at = [(i % cols) as f32 * STEP[0], (i / cols) as f32 * STEP[1]];
        ws.nodes.insert(format!("b{i}"), Node { block: block.into(), at, values, pins, ..Default::default() });
    }
    let link = |s: String| s.parse::<Link>().expect("a valid wire");
    for i in 1..n {
        // Flow wires (Math is a value block, so the flow skips over it).
        if i % 4 == 1 {
            continue;
        }
        let from = if i % 4 == 2 { i - 2 } else { i - 1 };
        let from_pin = if from % 4 == 2 { "true" } else { "next" };
        ws.links.push(link(format!("b{from}.{from_pin} > b{i}.in")));
        if i % 4 == 2 {
            ws.links.push(link(format!("b{}.result > b{i}.condition", i - 1)));
        }
        if i % 4 == 3 {
            ws.links.push(link(format!("b{}.result > b{i}.message", i - 2)));
        }
    }
    ws
}
