#include "controller.hpp"

class CheckoutController {
public:
    Response post(const Request &raw) {
        auto request = parser_.parse(raw);
        auto receipt = service_.submit(request);
        return Response::ok(receipt);
    }

private:
    OrderParser parser_;
    CheckoutService service_;
};
