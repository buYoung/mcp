#include "order.hpp"
#include <memory>
#include <vector>

template <typename T>
struct Result {
    T value;
};

class CheckoutService {
public:
    Result<OrderDto> submit(const std::vector<OrderLine> &lines) {
        auto dto = mapper_.map(lines);
        auto audit = AuditEvent::audit("checkout.submit", dto.total);

        inventory_.reserve(lines);
        repo_.save(dto);
        audit.commit();
        return Result<OrderDto>{dto};
    }

private:
    OrderMapper mapper_;
    Inventory inventory_;
    OrderRepo repo_;
};
