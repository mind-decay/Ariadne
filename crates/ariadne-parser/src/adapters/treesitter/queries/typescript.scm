; tier-03 syntactic-fact query — TypeScript (non-TSX).
; See javascript.scm for the capture scheme.
; node-type reference: https://github.com/tree-sitter/tree-sitter-typescript/blob/master/typescript/src/node-types.json

(function_declaration
  name: (identifier) @name) @def.function

(generator_function_declaration
  name: (identifier) @name) @def.function

(class_declaration
  name: (type_identifier) @name) @def.class

(method_definition
  name: (property_identifier) @name) @def.method

(interface_declaration
  name: (type_identifier) @name) @def.interface

(enum_declaration
  name: (identifier) @name) @def.enum

(type_alias_declaration
  name: (type_identifier) @name) @def.type

(lexical_declaration
  (variable_declarator
    name: (identifier) @name)) @def.variable

(variable_declaration
  (variable_declarator
    name: (identifier) @name)) @def.variable

(import_statement
  source: (string) @import.path) @import

(call_expression
  function: (identifier) @call.callee)

(call_expression
  function: (member_expression
    property: (property_identifier) @call.callee))

; --- Hooks (tier-02) --------------------------------------------------------
; Non-TSX TypeScript has no JSX nodes, so no `@render.component` capture here;
; hooks / reactive primitives can still be called from plain `.ts`. Capture
; scheme is documented in tsx.scm.

(call_expression
  function: (identifier) @hook.callee
  (#match? @hook.callee "^(use[A-Z]|createSignal|createEffect|createMemo|createResource)"))

; tier-04 visibility / attribute captures.
;   @visibility  `export_statement` wrapping a top-level decl; class
;                members' TS `accessibility_modifier` (`public` /
;                `protected` / `private`). `facts.rs` folds the wrapping
;                range / containment onto the inner decl.
;   @attribute   `decorator` nodes precede a class member; bind to next
;                decl in `attach_attributes`.

(export_statement) @visibility

(accessibility_modifier) @visibility

(decorator) @attribute
