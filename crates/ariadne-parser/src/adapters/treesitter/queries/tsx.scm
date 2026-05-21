; tier-02 syntactic-fact query — TSX (TypeScript + JSX).
;
; The decl/import/call captures are identical to typescript.scm: the TSX
; grammar's non-JSX node types are the TypeScript grammar's node types
; (the grammars differ only in how `<...>` is resolved).
; node-type reference: https://github.com/tree-sitter/tree-sitter-typescript/blob/master/tsx/src/node-types.json
;
; JSX additions and the build-session capture choices (plan tier-02 step 5):
;   @render.component  the tag-name `identifier` of every JSX opening /
;                      self-closing element — host *and* component. facts.rs
;                      classifies it: a capitalised name is a child component
;                      (`RenderSite`); a lower-case name is a host element
;                      (`div`). The capture is NOT `#match?`-filtered because
;                      every JSX tag, capitalised or not, also marks JSX
;                      presence for the component post-filter below.
;   @def.component     NOT a query capture. A tree-sitter pattern cannot
;                      express "body returns JSX at any depth", so facts.rs
;                      re-tags a `function_declaration` or a variable
;                      declaration (the `const Foo = () => <jsx/>` arrow form)
;                      whose `def_byte_range` encloses a JSX tag as
;                      `DeclKind::Component`.
;   @hook.callee       a `call_expression` callee `identifier` matching the
;                      hook convention — `use<Upper>` (React) or a SolidJS
;                      reactive primitive — filtered by the `#match?`
;                      predicate, which the Rust binding evaluates while
;                      iterating matches against the source text.

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

; --- JSX --------------------------------------------------------------------

(jsx_opening_element
  name: (identifier) @render.component)

(jsx_self_closing_element
  name: (identifier) @render.component)

(call_expression
  function: (identifier) @hook.callee
  (#match? @hook.callee "^(use[A-Z]|createSignal|createEffect|createMemo|createResource)"))
