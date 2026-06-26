import { Response } from "./http";
import { parseOrder } from "./parser";

type RequestBody = { body: unknown };

export class CheckoutController {
  constructor(private readonly service: CheckoutService) {}

  async post(request: RequestBody) {
    const payload = parseOrder(request.body);
    const result = await this.service.submit(payload);
    return Response.ok(result);
  }
}
