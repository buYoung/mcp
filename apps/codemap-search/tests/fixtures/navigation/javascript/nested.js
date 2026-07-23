import { createAudit } from "./audit";
import { OrderRepository } from "./repo";

export class CheckoutService {
  constructor(mapper, repo) {
    this.mapper = mapper;
    this.repo = repo;
  }

  submit(lines) {
    const dto = this.mapper.mapOrder(lines);
    const audit = createAudit("checkout.submit", dto);
    const total = Number(dto.total);

    this.repo.persist(dto);
    audit.track(total);
    return this.toDto(dto);
  }

  toDto(dto) {
    return dto;
  }
}
