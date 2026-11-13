"use client";
import { useCallback, useRef, useState } from "react";

export interface ToastItem { id: number; msg: string; kind: "good" | "bad" | "info"; link?: string | null }

export function useToasts() {
  const [items, setItems] = useState<ToastItem[]>([]);
  const n = useRef(0);
  const push = useCallback((msg: string, kind: "good" | "bad" | "info" = "info", link?: string | null) => {
    const id = ++n.current;
    setItems((c) => [...c.slice(-3), { id, msg, kind, link }]);
    setTimeout(() => setItems((c) => c.filter((t) => t.id !== id)), kind === "bad" ? 7000 : 4500);
  }, []);
  return { items, push };
}

export function Toasts({ items }: { items: ToastItem[] }) {
  return (
    <div className="toasts" role="status" aria-live="polite">
      {items.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`}>
          {t.msg}
          {t.link && (
            <>
              {" "}
              <a href={t.link} target="_blank" rel="noreferrer">view tx ↗</a>
            </>
          )}
        </div>
      ))}
    </div>
  );
}
