import { useMemo } from "react";
import { formatMoney } from "../money";

type LineItem = Readonly<{ id: string; amount: number }>;

export function OrderTable({
  items,
  onSelect,
}: {
  items: LineItem[];
  onSelect(id: string): void;
}) {
  const rows = useMemo(
    () =>
      items.map((item) => ({
        id: item.id,
        label: formatMoney(item.amount),
      })),
    [items],
  );

  return (
    <section>
      {rows.map((row) => (
        <button key={row.id} onClick={() => onSelect(row.id)}>
          {row.label}
        </button>
      ))}
    </section>
  );
}
