use tracing::warn;

pub struct Worker<Q, P> {
    queue: Q,
    processor: P,
}

impl<Q, P> Worker<Q, P>
where
    Q: JobQueue,
    P: JobProcessor,
{
    pub async fn tick(&self) -> Result<(), Error> {
        let Some(job) = self.queue.next().await? else {
            warn!("no job");
            return Ok(());
        };

        if let Some(retry) = self.queue.retry_hint().await? {
            warn!("retrying {retry}");
        }

        let receipt = self.processor.process(job).await?;
        self.queue.ack(receipt.id).await
    }
}
