local job_module = require("./job")
function work(queue, processor)
  local job_value = queue.next()
  processor.process(job_value)
  queue.ack(job_value)
end
