; tier-03 syntactic-fact query — JavaScript.
; Captures:
;   @def.<kind>  the declaration node (kind tag = suffix after `.`)
;   @name        the declared symbol's identifier
;   @import      the whole import statement
;   @import.path the raw module string (with surrounding quotes)
;   @call.callee the callee identifier of a call expression
; node-type reference: https://github.com/tree-sitter/tree-sitter-javascript/blob/master/src/node-types.json

(function_declaration
  name: (identifier) @name) @def.function

(generator_function_declaration
  name: (identifier) @name) @def.function

(class_declaration
  name: (identifier) @name) @def.class

(method_definition
  name: (property_identifier) @name) @def.method

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

; --- JSX (tier-02) ----------------------------------------------------------
; `.jsx` parses with the JavaScript grammar, which emits JSX nodes natively;
; plain `.js` has no JSX so these patterns are inert there. Capture scheme and
; the component post-filter are documented in tsx.scm.

(jsx_opening_element
  name: (identifier) @render.component)

(jsx_self_closing_element
  name: (identifier) @render.component)

(call_expression
  function: (identifier) @hook.callee
  (#match? @hook.callee "^(use[A-Z]|createSignal|createEffect|createMemo|createResource)"))
