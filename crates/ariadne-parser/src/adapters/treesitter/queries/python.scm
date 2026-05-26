; tier-03 syntactic-fact query — Python.
; node-type reference: https://github.com/tree-sitter/tree-sitter-python/blob/master/src/node-types.json

(function_definition
  name: (identifier) @name) @def.function

(class_definition
  name: (identifier) @name) @def.class

(import_statement
  name: (dotted_name) @import.path) @import

(import_from_statement
  module_name: (dotted_name) @import.path) @import

(call
  function: (identifier) @call.callee)

(call
  function: (attribute
    attribute: (identifier) @call.callee))

; tier-04 attribute captures (Python has no visibility keyword;
; underscore/dunder conventions are captured via the decl `@name` and
; classified downstream).
;   @attribute   `decorator` nodes preceding a function / class.

(decorator) @attribute
