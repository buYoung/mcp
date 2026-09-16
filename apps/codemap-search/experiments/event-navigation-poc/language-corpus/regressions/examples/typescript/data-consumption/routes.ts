class Queue { items: any[] = []; }
function route(queue: Queue, client: number, other: number, handler: () => void) {
  const queues = new Map<string, Queue>();
  const clients = new Map<string, number>();
  const handlers = new Map<number, () => void>();
  queues.set("inbox", queue); // @QUEUE
  const found = queues.get("inbox");
  found.items.push("message"); // @WRITE
  const separate = queues.get("other");
  separate.items.push("message"); // @OTHER_WRITE
  clients.set("request", client); // @CLIENT
  const id = clients.get("request");
  handlers.get(id); // @LOOKUP
  handlers.get(other); // @OTHER_LOOKUP
}
