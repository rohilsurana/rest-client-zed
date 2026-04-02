(request
  (request_line
    method: (method) @_method
    url: (url) @_url) @run
  (#set! tag "http-request")
  (#set! label "Send Request"))
