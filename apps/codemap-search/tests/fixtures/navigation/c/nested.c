#include "order.h"
#include <stdbool.h>

#define TRACE_ORDER(name, dto) audit_trace((name), (dto).total)

struct order_dto {
    int total;
};

static struct order_dto map_order(const struct order_line *lines, int count) {
    struct order_dto dto = {0};
    for (int i = 0; i < count; i++) {
        dto.total += lines[i].price * lines[i].quantity;
    }
    return dto;
}

bool checkout_submit(struct order_repo *repo, const struct order_line *lines, int count) {
    struct order_dto dto = map_order(lines, count);
    bool saved = false;

    reserve_inventory(lines, count);
    saved = repo_save(repo, &dto);
    TRACE_ORDER("checkout.submit", dto);
    return saved;
}
