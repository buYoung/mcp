import { useEffect, useState } from "react";

export function OrderDashboard({ client }: { client: OrderClient }) {
  const [orders, setOrders] = useState<OrderSummary[]>([]);

  useEffect(() => {
    client.load().then((items) => setOrders(items));
  }, [client]);

  return (
    <ul>
      {orders.map((order) => (
        <li key={order.id}>{order.status}</li>
      ))}
    </ul>
  );
}
