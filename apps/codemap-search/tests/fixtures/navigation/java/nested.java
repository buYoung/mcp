import java.util.List;
import org.springframework.stereotype.Service;

@Service
class CheckoutService {
    private final OrderRepository repository;
    private final Inventory inventory;
    private final OrderMapper mapper;

    CheckoutService(OrderRepository repository, Inventory inventory, OrderMapper mapper) {
        this.repository = repository;
        this.inventory = inventory;
        this.mapper = mapper;
    }

    @Transactional
    Receipt submit(List<OrderLine> lines) {
        OrderDto dto = mapper.map(lines);
        AuditEvent event = AuditEvent.audit("checkout.submit", dto.total());

        inventory.reserve(lines);
        repository.save(dto);
        event.commit();
        return Receipt.from(dto);
    }

    static final class Receipt {
        static Receipt from(OrderDto dto) {
            return new Receipt();
        }
    }
}
