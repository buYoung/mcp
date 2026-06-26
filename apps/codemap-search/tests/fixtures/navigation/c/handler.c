#include "handler.h"

int handle_checkout(struct http_request *raw, struct http_response *out) {
    struct submit_request request = parse_order(raw);
    struct receipt receipt = checkout_submit(&request);
    return write_response(out, &receipt);
}
