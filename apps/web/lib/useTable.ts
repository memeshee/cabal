"use client";
import { useCallback, useEffect, useRef, useState } from "react";
import { getTable, Gone } from "./api";
import type { ChatLine, Entry, TableView } from "./types";

/** Polls the house every second and merges the incremental feed/chat into one stable view. */
export function useTable(key: string) {
  const [view, setView] = useState<TableView | null>(null);
  const [feed, setFeed] = useState<Entry[]>([]);
  const [chat, setChat] = useState<ChatLine[]>([]);
  const [gone, setGone] = useState(false);
  const [lost, setLost] = useState(0);
  const counts = useRef({ feed: 0, chat: 0 });
  const stopped = useRef(false);

  const addChat = useCallback((lines: ChatLine[]) => {
    setChat((cur) => {
      const have = new Set(cur.map((c) => c.id));
      const add = lines.filter((l) => !have.has(l.id));
      if (!add.length) return cur;
      const next = [...cur, ...add].sort((a, b) => a.id - b.id);
      counts.current.chat = next.length;
      return next;
    });
  }, []);

  useEffect(() => {
    stopped.current = false;
    let timer: ReturnType<typeof setTimeout>;
    const tick = async () => {
      try {
        const v = await getTable(key, counts.current.feed, counts.current.chat);
        setView(v);
        setLost(0);
        if (v.feed.length) {
          setFeed((cur) => {
            const next = [...cur];
            for (const e of v.feed) if (e.idx === next.length) next.push(e);
            counts.current.feed = next.length;
            return next;
          });
        }
        if (v.chat.length) addChat(v.chat);
        if (v.phase === "revealed" || v.phase === "voided" || v.phase === "failed") {
          // One more pass is enough to pick up the last events; then stop polling.
          if (stopped.current) return;
          stopped.current = true;
          timer = setTimeout(tick, 1500);
          return;
        }
      } catch (e) {
        if (e instanceof Gone) {
          setGone(true);
          return;
        }
        setLost((n) => n + 1);
      }
      if (!stopped.current) timer = setTimeout(tick, 1000);
    };
    tick();
    return () => {
      stopped.current = true;
      clearTimeout(timer);
    };
  }, [key, addChat]);

  // Hand callers a view whose feed/chat are the full accumulated lists.
  const full: TableView | null = view ? { ...view, feed, chat, feed_total: feed.length, chat_total: chat.length } : null;
  return { view: full, gone, lost, addChat };
}
