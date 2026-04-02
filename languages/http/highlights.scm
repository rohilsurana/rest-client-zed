(method) @keyword

(url (url_segment) @string.special)

(http_version) @constant

(header
  name: (header_name) @property)

(header
  value: (header_value
    (header_value_segment) @string))

(comment) @comment

(comment_text) @comment

(request_separator) @punctuation.delimiter

(separator_label) @label

(variable
  (variable_name) @variable.special)

(system_variable
  "$" @punctuation.special
  name: (system_variable_name) @function.builtin)

(system_variable_args) @string

(file_variable
  "@" @punctuation.special
  name: (file_variable_name) @variable)

(file_variable_value
  (file_variable_raw) @string)

(annotation
  "@" @punctuation.special
  name: (annotation_name) @attribute)

(annotation_value) @string

(file_reference
  path: (file_path) @string.special)

(url_continuation
  (url_segment) @string.special)

(header_continuation
  value: (header_value
    (header_value_segment) @string))

(body_content) @string
