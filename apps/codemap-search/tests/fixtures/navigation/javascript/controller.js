import { Response } from "./http";
import { parseOrder } from "./parser";

export class CheckoutController {
  constructor(service) {
    this.service = service;
  }

  async post(request) {
    const payload = parseOrder(request.body);
    const result = await this.service.submit(payload);
    return Response.ok(result);
  }
}
