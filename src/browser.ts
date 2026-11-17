import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Everything the UI needs to know about one tab. */
export type Tab = {
  id: string;
  title: string;
  url: string;
  loading: boolean;
  pinned: boolean;
  muted: boolean;
  isPrivate: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
};

export type SearchEngine = {
  name: string;
  keyword: string;
  template: string;
};

export type Settings = {
  theme: string;
  defaultEngine: string;
  homepage: string;
  downloadDir: string | null;
  startupBehavior: string;
  showBookmarksBar: boolean;
  blockPopups: boolean;
};

export type Suggestion =
  | { kind: "openTab"; title: string; url: string; tabId: string }
  | { kind: "bookmark"; title: string; url: string }
  | { kind: "history"; title: string; url: string; visitCount: number }
  | { kind: "search"; query: string; engine: string; url: string };

export type HistoryEntry = {
  id: number;
  title: string | null;
  url: string;
  visitCount: number;
  lastVisitedAt: string;
};

export type Bookmark = {
  id: string;
  title: string;
  url: string;
  folder: string | null;
  createdAt: string;
};

export type DownloadItem = {
  id: string;
  url: string;
  filename: string;
  path: string;
  status: string;
  totalBytes: number | null;
  receivedBytes: number;
};

export type Viewport = { x: number; y: number; width: number; height: number };

export const HOME_URL = "browser://home";

/** True for the built-in start page and any empty/new-tab state. */
export function isHomeUrl(url?: string | null): boolean {
  return !url || url === "about:blank" || url === HOME_URL || url === "about:home";
}

/** Secure-connection indicator for the address bar. */
export function isSecureUrl(url: string): boolean {
  return url.startsWith("https://");
}

/** Strip the scheme and render a short host for display in the omnibox. */
export function displayUrl(url: string): string {
  if (isHomeUrl(url)) return "";
  try {
    const u = new URL(url);
    const path = u.pathname === "/" ? "" : u.pathname + u.search;
    return (u.host + path).replace(/^www\./, "");
  } catch {
    return url;
  }
}

/**
 * The browser state store.
 *
 * - Tabs/settings arrive from the backend via `tabs-changed` events; the UI
 *   never polls and never recomputes state it does not own.
 * - The content viewport is measured here once and re-measured on layout
 *   changes; `content_ready` / `update_viewport` tell the backend where to
 *   put the live webview.
 */
export function useBrowser() {
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [engines, setEngines] = useState<SearchEngine[]>([]);
  const [ready, setReady] = useState(false);
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const activeIdRef = useRef<string | null>(null);
  activeIdRef.current = activeId;

  const activeTab = tabs.find((t) => t.id === activeId) ?? null;

  // --- backend events ---
  useEffect(() => {
    let alive = true;
    const unlistens: Promise<UnlistenFn>[] = [
      listen<{ tabs: Tab[]; activeTabId: string | null }>("tabs-changed", (e) => {
        setTabs(e.payload.tabs);
        setActiveId(e.payload.activeTabId);
      }),
      listen<{ tabId: string; url: string }>("load-finished", (e) => {
        // The backend already updated tab state; this is a nudge for any
        // optimistic UI (spinner removal is driven by tabs-changed).
        void e;
      }),
    ];
    (async () => {
      try {
        const snap = await invoke<{
          tabs: Tab[];
          activeTabId: string | null;
          settings: Settings;
          engines: SearchEngine[];
        }>("get_snapshot");
        if (!alive) return;
        setTabs(snap.tabs);
        setActiveId(snap.activeTabId);
        setSettings(snap.settings);
        setEngines(snap.engines);
      } catch (err) {
        console.error("snapshot failed", err);
      } finally {
        if (alive) setReady(true);
      }
    })();
    return () => {
      alive = false;
      for (const p of unlistens) p.then((f) => f());
    };
  }, []);

  // --- viewport measurement ---
  const measureViewport = useCallback((): Viewport | null => {
    const el = viewportRef.current;
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: r.left, y: r.top, width: r.width, height: r.height };
  }, []);

  /** Tell the backend to (re)place the active tab's webview. */
  const syncContent = useCallback(async () => {
    const vp = measureViewport();
    if (!vp || vp.width < 50 || vp.height < 50) return;
    try {
      await invoke("content_ready", { viewport: vp });
    } catch (e) {
      console.error("content_ready failed", e);
    }
  }, [measureViewport]);

  /** Viewport changed size/position (window resize, panel toggle). */
  const syncBounds = useCallback(async () => {
    const vp = measureViewport();
    if (!vp || vp.width < 50 || vp.height < 50) return;
    try {
      await invoke("update_viewport", { viewport: vp });
    } catch {
      /* webview may not exist yet; fine */
    }
  }, [measureViewport]);

  // Track the active tab: materialize its webview whenever it changes.
  useEffect(() => {
    if (!ready) return;
    void syncContent();
  }, [ready, activeId, syncContent]);

  // Track viewport size changes (ResizeObserver keeps panel toggles in sync).
  useEffect(() => {
    if (!ready) return;
    const el = viewportRef.current;
    if (!el) return;
    let raf = 0;
    const onResize = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => void syncBounds());
    };
    const observer = new ResizeObserver(onResize);
    observer.observe(el);
    window.addEventListener("resize", onResize);
    return () => {
      cancelAnimationFrame(raf);
      observer.disconnect();
      window.removeEventListener("resize", onResize);
    };
  }, [ready, syncBounds]);

  // --- actions (thin wrappers; backend owns the state machine) ---
  const actions = {
    async newTab(url?: string, isPrivate = false, background = false) {
      try {
        await invoke("new_tab", { url, isPrivate, background });
      } catch (e) {
        console.error(e);
      }
    },
    async closeTab(id: string) {
      try {
        await invoke("close_tab", { id });
      } catch (e) {
        console.error(e);
      }
    },
    async selectTab(id: string) {
      try {
        await invoke("select_tab", { id });
      } catch (e) {
        console.error(e);
      }
    },
    async navigate(input: string, tabId?: string) {
      const vp = measureViewport();
      try {
        await invoke("navigate", { id: tabId, input, viewport: vp });
      } catch (e) {
        console.error(e);
      }
    },
    async goBack(tabId?: string) {
      const vp = measureViewport();
      try {
        await invoke("go_back", { id: tabId, viewport: vp });
      } catch (e) {
        console.error(e);
      }
    },
    async goForward(tabId?: string) {
      const vp = measureViewport();
      try {
        await invoke("go_forward", { id: tabId, viewport: vp });
      } catch (e) {
        console.error(e);
      }
    },
    async reload(tabId?: string) {
      const vp = measureViewport();
      try {
        await invoke("reload", { id: tabId, viewport: vp });
      } catch (e) {
        console.error(e);
      }
    },
    async stop(tabId?: string) {
      try {
        await invoke("stop", { id: tabId });
      } catch (e) {
        console.error(e);
      }
    },
    async togglePin(id: string) {
      try {
        await invoke("toggle_tab_pin", { id });
      } catch (e) {
        console.error(e);
      }
    },
    async toggleMute(id: string) {
      try {
        await invoke("toggle_tab_mute", { id });
      } catch (e) {
        console.error(e);
      }
    },
    async moveTab(id: string, toIndex: number) {
      try {
        await invoke("move_tab", { id, toIndex });
      } catch (e) {
        console.error(e);
      }
    },
    async reopenTab(url?: string, isPrivate?: boolean) {
      try {
        await invoke("reopen_tab", { url, isPrivate });
      } catch (e) {
        console.error(e);
      }
    },
    async suggest(input: string): Promise<Suggestion[]> {
      if (!input.trim()) return [];
      try {
        return await invoke<Suggestion[]>("omnibox_suggest", { input });
      } catch {
        return [];
      }
    },
    async listHistory(limit = 100): Promise<HistoryEntry[]> {
      try {
        return await invoke<HistoryEntry[]>("list_history", { limit });
      } catch {
        return [];
      }
    },
    async searchHistory(query: string): Promise<HistoryEntry[]> {
      try {
        return await invoke<HistoryEntry[]>("search_history", { query });
      } catch {
        return [];
      }
    },
    async clearHistory() {
      try {
        await invoke("clear_history");
      } catch (e) {
        console.error(e);
      }
    },
    async listBookmarks(): Promise<Bookmark[]> {
      try {
        return await invoke<Bookmark[]>("list_bookmarks");
      } catch {
        return [];
      }
    },
    async addBookmark(title: string, url: string) {
      try {
        await invoke("add_bookmark", { title, url, folder: null });
        return true;
      } catch (e) {
        console.error(e);
        return false;
      }
    },
    async removeBookmark(id: string) {
      try {
        await invoke("remove_bookmark", { id });
      } catch (e) {
        console.error(e);
      }
    },
    async isBookmarked(url: string): Promise<boolean> {
      try {
        return await invoke<boolean>("is_bookmarked", { url });
      } catch {
        return false;
      }
    },
    async listDownloads(): Promise<DownloadItem[]> {
      try {
        return await invoke<DownloadItem[]>("list_downloads");
      } catch {
        return [];
      }
    },
    async saveSettings(next: Settings) {
      try {
        await invoke("save_settings", { settings: next });
        setSettings(next);
        return true;
      } catch (e) {
        console.error(e);
        return false;
      }
    },
  };

  return {
    tabs,
    activeId,
    activeTab,
    settings,
    engines,
    ready,
    viewportRef,
    actions,
  };
}

export type BrowserActions = ReturnType<typeof useBrowser>["actions"];
