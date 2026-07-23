.include "http.inc"
.globl handle_request
handle_request:
  call parse_order
  call submit_order
  call response_ok
  ret
