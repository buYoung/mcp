import org.springframework.scheduling.annotation.Scheduled

class CheckoutWorker(
    private val queue: JobQueue,
    private val processor: JobProcessor,
    private val logger: Logger,
) {
    @Scheduled(fixedDelay = 1000)
    fun tick() {
        val job = queue.next()
        if (job == null) {
            logger.warn("no job")
            return
        }

        val receipt = processor.process(job)
        queue.ack(receipt.id)
    }
}
