<?php
use Demo\Repository as Repo;
function run(Mapper $mapper, Repo $repo): void
{
    $dto = $mapper->map();
    $repo->save($dto);
}

