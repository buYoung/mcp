<?php
use Demo\Job;
function work(Queue $queue, Processor $processor): void
{
    $job = $queue->next();
    $processor->process($job);
    $queue->ack($job);
}

