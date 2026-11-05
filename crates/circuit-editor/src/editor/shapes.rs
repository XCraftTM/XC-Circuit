//! Block shapes and the names blocks get from earlier ones, kept until what they're made from changes. A fingerprint per block tells.

use super::*;

/// Block shapes, kept until what they are made from changes.
#[derive(Default)]
pub struct ShapeCache {
    /// By workspace, then block: a fingerprint of what the shape was made
    /// from, and the shape (shared, so handing it out copies nothing).
    pub(super) blocks: HashMap<String, HashMap<String, (u64, Arc<BlockShape>)>>,
    /// Per workspace: the names each block gets from earlier blocks.
    pub(super) known: HashMap<String, Known>,
    /// Counts passes over the blocks; see `begin`.
    pub(super) pass: u64,
}

impl ShapeCache {
    /// Starts a pass over the blocks (a frame, or a lookup): the workspace's
    /// structure is checked once per pass, not once per block.
    pub fn begin(&mut self) {
        self.pass += 1;
    }

    pub fn clear(&mut self) {
        self.blocks.clear();
        self.known.clear();
    }

    pub fn forget_workspace(&mut self, key: &str) {
        self.blocks.remove(key);
        self.known.remove(key);
    }

    pub fn forget(&mut self, ws_key: &str, node: &str) {
        if let Some(blocks) = self.blocks.get_mut(ws_key) {
            blocks.remove(node);
        }
    }

    pub(super) fn get(&self, ws_key: &str, node: &str) -> Option<&BlockShape> {
        self.blocks.get(ws_key)?.get(node).map(|(_, s)| &**s)
    }

    /// The workspace's names, emptied when what they depend on changed.
    pub(super) fn known(&mut self, reg: &Registry, ws_key: &str, ws: &Workspace, project: &Value) -> &mut Known {
        let pass = self.pass;
        if !self.known.contains_key(ws_key) {
            self.known.insert(ws_key.to_string(), Known::default());
        }
        let k = self.known.get_mut(ws_key).expect("just inserted");
        if k.pass != pass || k.names.is_empty() {
            let (structure, project) = (structure_hash(reg, ws), text_hash(project));
            if structure != k.structure || project != k.project {
                k.names.clear();
                k.wiring = None;
                (k.structure, k.project) = (structure, project);
            }
            k.into.clear();
            for (i, l) in ws.links.iter().enumerate() {
                k.into.entry(l.to.node.clone()).or_default().push(i);
            }
            k.pass = pass;
        }
        k
    }
}

/// The names blocks in one workspace get from earlier blocks, and what they
/// were worked out from.
#[derive(Default)]
pub(super) struct Known {
    /// The pass in which `structure` was last checked.
    pub(super) pass: u64,
    /// A hash of the wires, the blocks, and the settings of event and
    /// variable blocks: everything the names depend on.
    pub(super) structure: u64,
    /// A hash of the project data that `define()` sees.
    pub(super) project: u64,
    /// Each block's names, and a hash of them.
    pub(super) names: HashMap<String, (Vec<String>, u64)>,
    /// The wires into each block, by their index in the workspace's links;
    /// built once per pass, so a block finds its own without a search.
    pub(super) into: HashMap<String, Vec<usize>>,
    /// The wires by block, for walking back along them; kept while the
    /// structure stays the same.
    pub(super) wiring: Option<crate::editor::values::Wiring>,
}

pub(super) fn text_hash(v: &impl serde::Serialize) -> u64 {
    let mut h = DefaultHasher::new();
    serde_json::to_string(v).unwrap_or_default().hash(&mut h);
    h.finish()
}

/// A fast hasher for the fingerprints above (FxHash: one multiply per
/// word). They only tell "changed" from "the same", so they needn't resist
/// attacks the way the standard hasher does.
#[derive(Default)]
pub(crate) struct Fingerprint(u64);

impl Hasher for Fingerprint {
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for c in chunks {
            self.add(u64::from_le_bytes(*c));
        }
        if !rest.is_empty() {
            let mut last = [0u8; 8];
            last[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(last) ^ rest.len() as u64);
        }
    }
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64);
    }
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

impl Fingerprint {
    pub(super) fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

/// Hashes a JSON value as it is, without writing it out as text first.
pub(super) fn hash_value(v: &Value, h: &mut impl Hasher) {
    match v {
        Value::Null => 0u8.hash(h),
        Value::Bool(b) => (1u8, b).hash(h),
        Value::Number(n) => (2u8, n.as_i64(), n.as_u64(), n.as_f64().map(f64::to_bits)).hash(h),
        Value::String(s) => (3u8, s).hash(h),
        Value::Array(a) => {
            (4u8, a.len()).hash(h);
            a.iter().for_each(|x| hash_value(x, h));
        }
        Value::Object(o) => hash_map(o, h),
    }
}

pub(super) fn hash_map(o: &Map<String, Value>, h: &mut impl Hasher) {
    (5u8, o.len()).hash(h);
    for (k, x) in o {
        k.hash(h);
        hash_value(x, h);
    }
}

/// What the names blocks get from earlier blocks depend on.
pub(super) fn structure_hash(reg: &Registry, ws: &Workspace) -> u64 {
    let mut h = DefaultHasher::new();
    for l in &ws.links {
        (&l.from.node, &l.from.pin, &l.to.node, &l.to.pin).hash(&mut h);
    }
    for (id, n) in &ws.nodes {
        (id, &n.block, &n.zone).hash(&mut h);
        let event = reg.block(&n.block).is_some_and(|d| d.kind == circuit_core::BlockKind::Event);
        if event || n.block_id() == "core.set_variable" {
            hash_map(&n.values, &mut h);
        }
    }
    ws.zones.keys().for_each(|z| z.hash(&mut h));
    h.finish()
}

/// Works out the names a block gets from earlier blocks, which need no {name}
/// input (unless one is wired already), and returns a hash of them; the
/// names themselves are in the cache.
pub(super) fn known_names(reg: &Registry, engine: &BlockEngine, cache: &mut ShapeCache, ws_key: &str, ws: &Workspace, node_id: &str, project: &Value) -> u64 {
    let k = cache.known(reg, ws_key, ws, project);
    if let Some((_, hash)) = k.names.get(node_id) {
        return *hash;
    }
    let wiring = k.wiring.take().unwrap_or_else(|| crate::editor::values::Wiring::new(ws));
    // Only event blocks' shapes are needed for them, and events have nothing
    // before them, so this goes one level deep.
    let upstream = crate::editor::values::upstream_with(ws, node_id, reg, &wiring);
    let mut known = crate::editor::values::known_names(ws, &upstream, reg, &mut |id| {
        let is_event = ws.nodes.get(id).and_then(|n| reg.block(&n.block)).is_some_and(|d| d.kind == circuit_core::BlockKind::Event);
        if is_event { node_shape(reg, engine, cache, ws_key, ws, id, project) } else { None }
    });
    known.retain(|name| ws.links_into(node_id, name).next().is_none());
    let mut known: Vec<String> = known.into_iter().collect();
    known.sort();
    let mut h = DefaultHasher::new();
    known.hash(&mut h);
    let hash = h.finish();
    let k = cache.known(reg, ws_key, ws, project);
    k.names.insert(node_id.to_string(), (known, hash));
    k.wiring = Some(wiring);
    hash
}

/// The type of the output feeding a wire: from its block's shape when known
/// (`define()` can change it), else from the block's definition.
pub(super) fn wire_type<'a>(reg: &'a Registry, cache: &'a ShapeCache, ws_key: &str, ws: &Workspace, link: &Link) -> Option<&'a str> {
    let src = ws.nodes.get(&link.from.node)?;
    let cached = cache.get(ws_key, &link.from.node).and_then(|s| s.output(&link.from.pin)).map(|o| o.type_name.as_str());
    cached.or_else(|| reg.block(&src.block).and_then(|d| d.outputs.get(&link.from.pin)).map(|o| o.type_name.as_str()))
}

/// The effective shape of a node, re-running `define()` only when its values or
/// connected types change. Call `cache.begin()` before a pass over the blocks.
pub fn node_shape(reg: &Registry, engine: &BlockEngine, cache: &mut ShapeCache, ws_key: &str, ws: &Workspace, node_id: &str, project: &Value) -> Option<Arc<BlockShape>> {
    let node = ws.nodes.get(node_id)?;
    let def = reg.block(&node.block)?;
    let known_hash = known_names(reg, engine, cache, ws_key, ws, node_id, project);
    let project_hash = cache.known(reg, ws_key, ws, project).project;
    let into: &[usize] = cache.known.get(ws_key).and_then(|k| k.into.get(node_id)).map(Vec::as_slice).unwrap_or(&[]);
    let inputs = || into.iter().filter_map(|&i| ws.links.get(i)).filter(|l| l.to.node == node_id && l.to.pin != ACTION_IN);

    // A fingerprint of everything the shape is made from.
    let mut h = Fingerprint::default();
    node.block.hash(&mut h);
    hash_map(&node.values, &mut h);
    for link in inputs() {
        (&link.to.pin, wire_type(reg, cache, ws_key, ws, link)).hash(&mut h);
    }
    known_hash.hash(&mut h);
    if def.has_define {
        project_hash.hash(&mut h);
    }
    let hash = h.finish();
    if let Some((cached_hash, shape)) = cache.blocks.get(ws_key).and_then(|b| b.get(node_id))
        && *cached_hash == hash
    {
        return Some(shape.clone());
    }

    // Changed (or new): build it again.
    let mut connected = Map::new();
    for link in inputs() {
        if let Some(ty) = wire_type(reg, cache, ws_key, ws, link) {
            connected.insert(link.to.pin.clone(), Value::String(ty.to_string()));
        }
    }
    let known: HashSet<String> = cache.known.get(ws_key).and_then(|k| k.names.get(node_id)).map(|(v, _)| v.iter().cloned().collect()).unwrap_or_default();
    let define = if def.has_define {
        let engine_key = reg.engine_keys.get(&def.id)?;
        engine.define(engine_key, &node.values, &connected, project).ok()
    } else {
        None
    };
    let shape = Arc::new(BlockShape::build_with(def, &node.values, define.as_ref(), &known));
    cache.blocks.entry(ws_key.to_string()).or_default().insert(node_id.to_string(), (hash, shape.clone()));
    Some(shape)
}
