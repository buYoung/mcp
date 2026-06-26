import org.springframework.scheduling.annotation.Scheduled;

class CheckoutWorker {
    private final JobQueue queue;
    private final JobProcessor processor;
    private final Logger logger;

    @Scheduled(fixedDelay = 1000)
    void tick() {
        Job job = queue.next();
        if (job == null) {
            logger.warn("no job");
            return;
        }

        Receipt receipt = processor.process(job);
        queue.ack(receipt.id());
    }
}
