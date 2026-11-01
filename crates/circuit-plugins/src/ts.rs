//! TypeScript block files: types are stripped with oxc, nothing is type-checked
//! (your code editor does that with the `@xc-circuit/sdk` types).

use std::path::Path;

use anyhow::{Result, bail};
use oxc::allocator::Allocator;
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};

/// Converts a `.ts` (or `.js`) block file to plain JavaScript.
pub fn to_javascript(path: &Path, source: &str) -> Result<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts());
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if let Some(e) = parsed.diagnostics.first() {
        bail!("{e}");
    }
    let mut program = parsed.program;
    let scoping = SemanticBuilder::new().build(&program).semantic.into_scoping();
    let out = Transformer::new(&allocator, path, &TransformOptions::default()).build_with_scoping(scoping, &mut program);
    if let Some(e) = out.diagnostics.first() {
        bail!("{e}");
    }
    Ok(imports_to_require(&Codegen::new().build(&program).code))
}

/// `import { a, b } from "x"` → `const { a, b } = require("x")`, and default and
/// namespace imports likewise, so the file runs as CommonJS like other blocks.
pub fn imports_to_require(code: &str) -> String {
    code.lines()
        .map(|line| {
            let t = line.trim_start();
            let Some(rest) = t.strip_prefix("import ") else { return line.to_string() };
            let Some((what, from)) = rest.rsplit_once(" from ") else { return line.to_string() };
            let module = from.trim().trim_end_matches(';');
            let what = what.trim();
            if let Some(ns) = what.strip_prefix("* as ") {
                format!("const {} = require({module});", ns.trim())
            } else if what.starts_with('{') {
                format!("const {} = require({module});", what.replace(" as ", ": "))
            } else if let Some((def, named)) = what.split_once(',') {
                format!("const {} = require({module}); const {} = require({module});", def.trim(), named.trim().replace(" as ", ": "))
            } else {
                format!("const {what} = (m => m && m.default ? m.default : m)(require({module}));")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_types_and_rewrites_imports() {
        let src = r#"
            import { defineBlock } from "@xc-circuit/sdk";
            interface Fields { name: string }
            export default defineBlock({
              id: "t.greet", name: "Greet", description: "d", author: "a", version: "1.0.0", kind: "value",
              fields: { name: { type: "text" } },
              outputs: { text: { type: "text" } },
              run({ fields }: { fields: Fields }): { text: string } {
                const n: string = fields.name;
                return { text: `Hi ${n}` };
              },
            });
        "#;
        let js = to_javascript(Path::new("greet.ts"), src).unwrap();
        assert!(js.contains("require(\"@xc-circuit/sdk\")"), "{js}");
        assert!(!js.contains("interface") && !js.contains(": string"), "{js}");
    }

    #[test]
    fn reports_syntax_errors() {
        assert!(to_javascript(Path::new("bad.ts"), "export default { run( { }").is_err());
    }
}
