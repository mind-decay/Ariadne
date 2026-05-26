; tier-03 syntactic-fact query — Rust.
; node-type reference: https://github.com/tree-sitter/tree-sitter-rust/blob/master/src/node-types.json
;
; tier-04 captures (attached to decls in `facts.rs` via byte-range
; containment / proximity):
;   @visibility  the `visibility_modifier` token of a decl (`pub`,
;                `pub(crate)`, `pub(super)`, …) — its parent decl's
;                `def_byte_range` contains it, so it folds onto that decl
;                in `attach_visibility`.
;   @attribute   the `attribute_item` node (`#[test]`, `#[derive(Debug)]`,
;                …). Its byte range precedes the decl it annotates;
;                `attach_attributes` binds it to the next decl.

(function_item
  name: (identifier) @name) @def.function

(struct_item
  name: (type_identifier) @name) @def.struct

(enum_item
  name: (type_identifier) @name) @def.enum

(trait_item
  name: (type_identifier) @name) @def.trait

(type_item
  name: (type_identifier) @name) @def.type

(mod_item
  name: (identifier) @name) @def.module

(use_declaration
  argument: (_) @import.path) @import

(call_expression
  function: (identifier) @call.callee)

(call_expression
  function: (scoped_identifier
    name: (identifier) @call.callee))

(call_expression
  function: (field_expression
    field: (field_identifier) @call.callee))

(visibility_modifier) @visibility

(attribute_item) @attribute
