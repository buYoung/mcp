import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.http.ResponseEntity;

class CheckoutController {
    private final CheckoutService service;
    private final OrderParser parser;

    CheckoutController(CheckoutService service, OrderParser parser) {
        this.service = service;
        this.parser = parser;
    }

    @PostMapping("/checkout")
    ResponseEntity<Receipt> post(String body) {
        SubmitRequest request = parser.parse(body);
        Receipt receipt = service.submit(request);
        return ResponseEntity.ok(receipt);
    }
}
