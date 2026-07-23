<main>mixed html</main>
<?php
namespace Demo;

use Demo\Support\Clock as AppClock;
require_once './worker.php';

#[Deprecated]
class Worker
{
    public const STATE = 'ready';
    public string $status = "idle";

    public function run(AppClock $clock): void
    {
        helper();
        $clock->tick();
        AppClock::now();
        $child = new Worker();
    }
}

function helper(): void {}

enum Mode
{
    case Fast;
}

