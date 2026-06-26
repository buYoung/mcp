#include "worker.h"

void worker_tick(struct job_queue *queue, struct processor *processor) {
    struct job job = queue_next(queue);
    if (!job.valid) {
        log_warn("no job");
        return;
    }

    struct receipt receipt = process_job(processor, &job);
    queue_ack(queue, receipt.id);
}
