import logging


logger = logging.getLogger(__name__)


def tick(queue, processor):
    job = queue.next_job()
    if job is None:
        logger.warning("no job")
        return None

    receipt = processor.process(job)
    queue.ack(receipt.id)
    return receipt
