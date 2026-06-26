import com.example.OrderMapper
import org.springframework.stereotype.Service

@Service
class CheckoutService(
    private val repository: OrderRepository,
    private val inventory: Inventory,
    private val mapper: OrderMapper,
) {
    @Transactional
    fun submit(lines: List<OrderLine>): Receipt {
        val dto: OrderDto = mapper.map(lines)
        val event = AuditEvent.audit("checkout.submit", dto.total)

        inventory.reserve(lines)
        repository.save(dto)
        event.commit()
        return Receipt.from(dto)
    }

    companion object {
        fun empty(): Receipt = Receipt.from(OrderDto(0))
    }
}
