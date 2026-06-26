import org.springframework.web.bind.annotation.PostMapping

class CheckoutController(
    private val service: CheckoutService,
    private val parser: OrderParser,
) {
    @PostMapping("/checkout")
    fun post(body: String): ResponseEntity<Receipt> {
        val request = parser.parse(body)
        val receipt = service.submit(request)
        return ResponseEntity.ok(receipt)
    }
}
