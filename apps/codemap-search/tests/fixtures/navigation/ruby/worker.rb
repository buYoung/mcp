require_relative "job"
def work(queue, processor)
  job = queue.next()
  processor.process(job)
  queue.ack(job)
end

