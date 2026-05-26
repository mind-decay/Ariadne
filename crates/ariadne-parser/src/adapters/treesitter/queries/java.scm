; tier-03 syntactic-fact query — Java.
; node-type reference: https://github.com/tree-sitter/tree-sitter-java/blob/master/src/node-types.json

(class_declaration
  name: (identifier) @name) @def.class

(interface_declaration
  name: (identifier) @name) @def.interface

(enum_declaration
  name: (identifier) @name) @def.enum

(record_declaration
  name: (identifier) @name) @def.record

(method_declaration
  name: (identifier) @name) @def.method

(import_declaration
  (scoped_identifier) @import.path) @import

(method_invocation
  name: (identifier) @call.callee)

; tier-04 visibility / attribute captures.
;   @visibility  the `modifiers` node — its text contains every modifier
;                keyword on the decl; `attach_visibility` scans it for
;                `public` / `protected` / `private`.
;   @attribute   `annotation` (`@Override`) / `marker_annotation` (`@Test`)
;                — contained in the method/class `modifiers` so they bind
;                via byte-range containment in `attach_attributes`.

(modifiers) @visibility

(annotation) @attribute

(marker_annotation) @attribute
