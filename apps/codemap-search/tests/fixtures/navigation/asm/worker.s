.include "queue.inc"
.globl tick_worker
tick_worker:
  call queue_pending
  call queue_next
  call process_job
  call queue_ack
  call logger_warn
  ret
