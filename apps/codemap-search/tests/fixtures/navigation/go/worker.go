package checkout

import "context"

type Worker struct {
	queue     JobQueue
	processor Processor
	logger    Logger
}

func (w Worker) Tick(ctx context.Context) error {
	job, ok := w.queue.Next(ctx)
	if !ok {
		w.logger.Warn("no job")
		return nil
	}

	receipt, err := w.processor.Process(ctx, job)
	if err != nil {
		return err
	}
	return w.queue.Ack(ctx, receipt.ID)
}
