import { useMemo } from "react";

type MenuItem = { id: string; enabled: boolean; label: string };

export function ActionMenu({
  items,
  onChoose,
}: {
  items: MenuItem[];
  onChoose(id: string): void;
}) {
  const visible = useMemo(() => items.filter((item) => item.enabled), [items]);

  return (
    <nav>
      {visible.map((item) => (
        <button key={item.id} onClick={() => onChoose(item.id)}>
          {item.label}
        </button>
      ))}
    </nav>
  );
}
