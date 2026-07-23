<?php
use Demo\Request;
function handle(Service $service): void
{
    $request = Request::parse();
    $service->submit($request);
}

