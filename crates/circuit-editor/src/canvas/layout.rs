//! Where a block's rows and pins go, Try Zone outlines and their outputs, and fitting the view to the blocks.

use super::*;

#[derive(Clone, Debug)]
pub(super) struct PinSpot {
    pub(super) pin: String,
    pub(super) dir: Dir,
    pub(super) ty: PinType,
    pub(super) pos: Pos2,
    pub(super) label: String,
    pub(super) description: Option<String>,
    /// An inline field that can be turned into a pin; drawn faintly.
    pub(super) faint: bool,
    pub(super) multi: bool,
}

#[derive(Clone, Debug)]
pub(super) enum RowKind {
    Flow(String),
    Field { key: String, pinned: bool },
    Output(String),
}

#[derive(Clone, Debug)]
pub(super) struct NodeLayout {
    pub(super) id: String,
    pub(super) rect: Rect,
    pub(super) header: Rect,
    pub(super) value_block: bool,
    /// Shared, so a layout kept from an earlier frame is cheap to use again.
    pub(super) rows: std::sync::Arc<Vec<(RowKind, Rect)>>,
    pub(super) pins: std::sync::Arc<Vec<PinSpot>>,
}

/// `tall` gives rows that need more room than one line (text boxes, embed
/// previews), by field key: their label and pin stay on the first line.
/// `fit` gives rows made as tall as their control (a dropdown, say): their
/// label and pin are centered on it. `width` is the block's own width when it
/// was resized.
#[allow(clippy::too_many_arguments)]
pub(super) fn layout_node(id: &str, at: [f32; 2], pins_on: &[String], width: Option<f32>, def: Option<&BlockDef>, shape: Option<&BlockShape>, tall: &HashMap<String, f32>, fit: &HashMap<String, f32>, show_advanced: bool) -> NodeLayout {
    let (Some(def), Some(shape)) = (def, shape) else {
        let rect = Rect::from_min_size(pos2(at[0], at[1]), vec2(NODE_W, HEADER_H + ROW_H + PAD_BOTTOM));
        let header = Rect::from_min_size(rect.min, vec2(NODE_W, HEADER_H));
        return NodeLayout { id: id.into(), rect, header, value_block: false, rows: Default::default(), pins: Default::default() };
    };
    let value_block = def.kind == BlockKind::Value;
    let width = width.unwrap_or_else(|| default_width(def));
    let header_h = if value_block { VALUE_HEADER_H } else { HEADER_H };
    let origin = pos2(at[0], at[1]);
    let header = Rect::from_min_size(origin, vec2(width, header_h));

    let mut rows = Vec::new();
    let single_next = shape.flows.len() == 1 && shape.flows[0] == "next";
    if !single_next {
        rows.extend(shape.flows.iter().map(|f| RowKind::Flow(f.clone())));
    }
    for (key, f) in shape.visible_fields() {
        // Advanced fields show too (Settings › Editor › Advanced options on
        // blocks); with that off, they stay in the Inspector unless they
        // were made pins (there, or by connecting them).
        if f.advanced && !show_advanced && !pins_on.contains(key) {
            continue;
        }
        let pinned = f.pin == PinMode::Only || pins_on.contains(key);
        rows.push(RowKind::Field { key: key.clone(), pinned });
    }
    rows.extend(shape.outputs.iter().map(|(k, _)| RowKind::Output(k.clone())));

    let row_h = |row: &RowKind| match row {
        RowKind::Field { key, .. } => tall.get(key).or_else(|| fit.get(key)).copied().unwrap_or(ROW_H),
        _ => ROW_H,
    };
    let height = header_h + PAD_TOP + rows.iter().map(row_h).sum::<f32>() + PAD_BOTTOM;
    let rect = Rect::from_min_size(origin, vec2(width, height));
    let mut pins = Vec::new();
    let header_y = header.center().y;
    if shape.action_in {
        pins.push(PinSpot {
            pin: ACTION_IN.into(),
            dir: Dir::In,
            ty: PinType::Action,
            pos: pos2(rect.left(), header_y),
            label: "Action".into(),
            description: Some("Runs this block.".into()),
            faint: false,
            multi: true,
        });
    }
    if single_next {
        pins.push(PinSpot {
            pin: "next".into(),
            dir: Dir::Out,
            ty: PinType::Action,
            pos: pos2(rect.right(), header_y),
            label: "Action".into(),
            description: Some("Runs the next blocks when this one is done.".into()),
            faint: false,
            multi: true,
        });
    }

    let mut laid = Vec::new();
    let mut top = header.bottom() + PAD_TOP;
    for row in rows.into_iter() {
        let h = row_h(&row);
        let r = Rect::from_min_size(pos2(rect.left(), top), vec2(width, h));
        top += h;
        // Pins sit at the middle of the first line, even on tall rows; on rows
        // as tall as their control, at its middle.
        let centered = matches!(&row, RowKind::Field { key, .. } if fit.contains_key(key));
        let y = if centered { r.center().y } else { r.top() + ROW_H / 2.0 };
        match &row {
            RowKind::Flow(f) => pins.push(PinSpot {
                pin: f.clone(),
                dir: Dir::Out,
                ty: PinType::Action,
                pos: pos2(rect.right(), y),
                label: circuit_core::block::title_case(f),
                description: None,
                faint: false,
                multi: true,
            }),
            RowKind::Field { key, pinned } => {
                let f = shape.field(key).expect("visible field");
                if *pinned || f.pin != PinMode::Inline {
                    pins.push(PinSpot {
                        pin: key.clone(),
                        dir: Dir::In,
                        ty: f.ty.clone(),
                        pos: pos2(rect.left(), y),
                        label: f.label(key),
                        description: f.description.clone(),
                        faint: !*pinned,
                        multi: f.multi,
                    });
                }
            }
            RowKind::Output(k) => {
                let o = shape.output(k).expect("output");
                pins.push(PinSpot {
                    pin: k.clone(),
                    dir: Dir::Out,
                    ty: o.ty.clone(),
                    pos: pos2(rect.right(), y),
                    label: o.label(k),
                    description: o.description.clone(),
                    faint: false,
                    multi: true,
                });
            }
        }
        laid.push((row, r));
    }
    NodeLayout { id: id.into(), rect, header, value_block, rows: laid.into(), pins: pins.into() }
}

/// How wide a block is before it is resized.
pub(super) fn default_width(def: &BlockDef) -> f32 {
    // Blocks that show a message (an embed, a layout), a form or a picture get room for it.
    let shows = matches!(def.preview.as_deref(), Some("embed" | "canvas")) || def.fields.values().any(|f| matches!(f.type_name.as_str(), "layout" | "modal"));
    def.width.unwrap_or(if shows { EMBED_NODE_W } else { NODE_W })
}

/// How wide a block with an embed, layout or form preview gets by itself:
/// enough for the preview as Discord sizes it (as wide as its content, up to
/// Discord's widest), never narrower than it starts. None for other blocks.
/// Worked out again only when its values change (`cache`).
pub(super) fn auto_width(measure: &egui::Painter, cache: &mut HashMap<String, (u64, f32)>, id: &str, node: &circuit_core::Node, def: Option<&BlockDef>, shape: Option<&BlockShape>) -> Option<f32> {
    let (def, shape) = (def?, shape?);
    let previews: Vec<(&String, &circuit_core::FieldDef)> = shape.visible_fields().filter(|(_, f)| matches!(f.type_name.as_str(), "layout" | "modal" | "embed")).collect();
    if previews.is_empty() {
        return None;
    }
    let value = |key: &String, f: &circuit_core::FieldDef| node.values.get(key).or(f.default.as_ref()).cloned().unwrap_or(Value::Null);
    let mut h = crate::editor::Fingerprint::default();
    for (key, f) in &previews {
        (key.as_str(), value(key, f).to_string()).hash(&mut h);
    }
    let print = h.finish();
    if let Some((p, w)) = cache.get(id)
        && *p == print
    {
        return Some(*w);
    }
    let preview = previews
        .iter()
        .map(|(key, f)| {
            let v = value(key, f);
            match f.type_name.as_str() {
                "layout" => crate::builders::layout::block_preview_width(measure, &v),
                "modal" => crate::builders::modal::block_preview_width(measure, &v),
                _ => crate::builders::embed::layout(measure, &crate::builders::embed::EmbedData::from_value(&v), crate::builders::embed::MAX_WIDTH, Some(4), 1.0).width,
            }
        })
        .fold(0.0, f32::max);
    // The preview sits 14 px in from each side.
    let w = (preview + 28.0).max(default_width(def)).ceil();
    cache.insert(id.to_string(), (print, w));
    Some(w)
}

/// How tall a text box on a block is: as it was resized, or as it starts.
pub(super) fn text_box_h(node: &circuit_core::Node, key: &str, f: &circuit_core::FieldDef) -> f32 {
    node.heights.get(key).copied().unwrap_or(if f.type_name == "textarea" { TEXT_AREA_H } else { TEXT_BOX_H })
}

pub(super) const ZONE_PANEL_W: f32 = 150.0;

pub(super) const ZONE_ROW_H: f32 = 22.0;

pub(super) const ZONE_PANEL_H: f32 = 4.0 * ZONE_ROW_H + 8.0;

/// Rectangles of the zones that have blocks, fitted around them.
pub(super) fn zone_rects(ws: &Workspace, layouts: &[NodeLayout]) -> Vec<(String, Rect)> {
    ws.zones
        .keys()
        .filter_map(|zid| {
            let bounds = layouts
                .iter()
                .filter(|l| ws.nodes.get(&l.id).is_some_and(|n| n.zone.as_deref() == Some(zid)))
                .map(|l| l.rect)
                .reduce(|a, b| a.union(b))?;
            let rect = Rect::from_min_max(bounds.min - vec2(28.0, 44.0), bounds.max + vec2(28.0 + ZONE_PANEL_W * 0.4, 28.0 + ZONE_PANEL_H));
            Some((zid.clone(), rect))
        })
        .collect()
}

pub(super) fn zone_layout(zid: &str, rect: Rect) -> NodeLayout {
    let header = Rect::from_min_size(rect.min, vec2(rect.width(), 28.0));
    let top = rect.bottom() - 8.0 - ZONE_PANEL_H + 4.0;
    let outputs: [(&str, &str, PinType, &str); 4] = [
        ("done", "Done", PinType::Action, "Runs when every block inside finished without an error."),
        ("error", "Error", PinType::Action, "Runs when a block inside failed (after all retries)."),
        ("message", "Error message", PinType::Text, "What went wrong."),
        ("details", "Error details", PinType::Object, "Block name, error code, workspace and stack trace."),
    ];
    let pins = outputs
        .into_iter()
        .enumerate()
        .map(|(i, (pin, label, ty, description))| PinSpot {
            pin: pin.into(),
            dir: Dir::Out,
            ty,
            pos: pos2(rect.right(), top + ZONE_ROW_H * (i as f32 + 0.5)),
            label: label.into(),
            description: Some(description.into()),
            faint: false,
            multi: true,
        })
        .collect();
    NodeLayout { id: zid.into(), rect, header, value_block: false, rows: Default::default(), pins: std::sync::Arc::new(pins) }
}

/// Centers the view on every block and note, at 100% when they fit.
pub(super) fn fit_view(view: &mut CanvasView, outer: Rect, layouts: &[NodeLayout], ws: &Workspace) {
    view.initialized = true;
    view.zoom = 1.0;
    let Some(bounds) = layouts.iter().map(|l| l.rect).chain(ws.notes.values().map(note_rect)).reduce(|a, b| a.union(b)) else {
        view.offset = vec2(40.0, 40.0);
        return;
    };
    let bounds = bounds.expand(40.0);
    view.zoom = (outer.width() / bounds.width()).min(outer.height() / bounds.height()).clamp(0.5, 1.0);
    view.offset = outer.size() / 2.0 - bounds.center().to_vec2() * view.zoom;
}
