import { OrderRepository } from "./repository";
import { createAudit } from "./audit";

type OrderLine = Readonly<{ price: string; quantity: number }>;
type OrderDto = Pick<OrderLine, "price" | "quantity"> & { total: number };

function Trace(): MethodDecorator {
  return () => undefined;
}

class OrderMapper {
  mapOrder(lines: readonly OrderLine[]): OrderDto {
    const total = lines.reduce((sum, line) => sum + Number(line.price) * line.quantity, 0);
    return { price: lines[0]?.price ?? "0", quantity: lines.length, total } as OrderDto;
  }
}

export class CheckoutService {
  constructor(
    private readonly repo: OrderRepository,
    private readonly mapper: OrderMapper,
  ) {}

  @Trace()
  async submit(lines: readonly OrderLine[]) {
    const dto = this.mapper.mapOrder(lines);
    const audit = createAudit(dto.total);

    await this.repo.persist(this.toDto(dto));
    audit.track("checkout-submitted");
  }

  private toDto(dto: OrderDto) {
    return { ...dto, total: dto.total satisfies number };
  }
}
