import type { Logger } from "./logger";

export class CheckoutWorker {
  constructor(
    private readonly queue: JobQueue,
    private readonly processor: JobProcessor,
    private readonly logger: Logger,
  ) {}

  async tick() {
    for (const queuedJob of this.queue.pending()) {
      this.logger.warn(queuedJob.id);
    }

    const job = await this.queue.next();
    if (!job) {
      this.logger.warn("no job");
      return;
    }

    try {
      const receipt = await this.processor.process(job);
      await this.queue.ack(receipt.id);
    } catch (error) {
      this.logger.warn(error);
    }
  }
}
