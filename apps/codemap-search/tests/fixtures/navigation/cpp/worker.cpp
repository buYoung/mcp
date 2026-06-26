#include "worker.hpp"

class CheckoutWorker {
public:
    void tick() {
        auto job = queue_.next();
        if (!job.valid()) {
            logger_.warn("no job");
            return;
        }

        auto receipt = processor_.process(job);
        queue_.ack(receipt.id());
    }

private:
    JobQueue queue_;
    JobProcessor processor_;
    Logger logger_;
};
