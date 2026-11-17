import {
  memo, useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent,
} from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowLeft, ArrowRight, RotateCw, Home, Star, StarOff, Download, Settings as SettingsIcon,
  Plus, X, Search, History as HistoryIcon, Moon, Sun, Lock, Globe, Pin, VolumeX,
  Volume2, Clock, BookmarkIcon, Trash2, ExternalLink, ShieldCheck, EyeOff, Square,
} from "lucide-react";
import HomePage from "./Home";
import {
  useBrowser, isHomeUrl, isSecureUrl, displayUrl,
  type Tab, type Suggestion, type HistoryEntry, type Bookmark, type DownloadItem,
} from "./browser";

/* ------------------------------------------------------------------ */
/* Small shared bits                                                   */
/* ------------------------------------------------------------------ */

function IconButton(props: {
  title: string; onClick: (e: ReactMouseEvent) => void; disabled?: boolean; children: React.ReactNode;
  active?: boolean;
}) {
  return (
    <button
      type="button"
      title={props.title}
      onClick={props.onClick}
      disabled={props.disabled}
      className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg transition-colors
        ${props.disabled ? "opacity-30" : "hover:bg-black/10 dark:hover:bg-white/10"}
        ${props.active ? "text-sky-500" : ""}`}
    >
      {props.children}
    </button>
  );
}

/** Toast notifications. */
function Toasts({ toast }: { toast: { message: string; kind: "success" | "error" } | null }) {
  if (!toast) return null;
  return (
    <div
      role="status"
      className={`fixed bottom-4 right-4 z-50 rounded-xl px-4 py-2.5 text-sm font-medium text-white shadow-xl
        ${toast.kind === "success" ? "bg-emerald-600" : "bg-red-600"}`}
    >
      {toast.message}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Tab strip                                                           */
/* ------------------------------------------------------------------ */

const TabChip = memo(function TabChip(props: {
  tab: Tab;
  active: boolean;
  dark: boolean;
  onSelect: () => void;
  onClose: () => void;
  onPin: () => void;
  onMute: () => void;
  onContextMenu: (e: ReactMouseEvent) => void;
}) {
  const { tab, active } = props;
  return (
    <div
      onClick={props.onSelect}
      onAuxClick={(e) => { if (e.button === 1) { e.preventDefault(); props.onClose(); } }}
      onContextMenu={props.onContextMenu}
      title={`${tab.title}\n${tab.url}`}
      className={`group flex h-9 min-w-[120px] max-w-[240px] shrink-0 cursor-pointer select-none items-center gap-1.5 rounded-t-lg px-2.5 text-xs
        border-x border-t transition-colors
        ${active
          ? props.dark
            ? "border-zinc-700 bg-zinc-800 text-zinc-100"
            : "border-zinc-300 bg-white text-zinc-900"
          : props.dark
            ? "border-transparent bg-zinc-900/60 text-zinc-400 hover:bg-zinc-900"
            : "border-transparent bg-zinc-200/60 text-zinc-600 hover:bg-zinc-200"}`}
    >
      {tab.isPrivate ? <EyeOff size={12} className="shrink-0 text-violet-400" />
        : isSecureUrl(tab.url) ? <Lock size={12} className="shrink-0 text-emerald-500" />
        : <Globe size={12} className="shrink-0 opacity-50" />}
      {tab.pinned && <Pin size={11} className="shrink-0 text-amber-500" />}
      {tab.muted && <VolumeX size={11} className="shrink-0 text-red-400" />}
      {tab.loading && <span className="h-2 w-2 shrink-0 animate-pulse rounded-full bg-sky-400" />}
      <span className="min-w-0 flex-1 truncate">{tab.title || "New Tab"}</span>
      <button
        type="button"
        title="Close tab"
        onClick={(e) => { e.stopPropagation(); props.onClose(); }}
        className="flex h-5 w-5 shrink-0 items-center justify-center rounded opacity-0 transition-opacity hover:bg-black/20 dark:hover:bg-white/20 group-hover:opacity-100"
      >
        <X size={12} />
      </button>
    </div>
  );
});

/* ------------------------------------------------------------------ */
/* Omnibox                                                             */
/* ------------------------------------------------------------------ */

function Omnibox(props: {
  tab: Tab | null;
  engineName: string;
  dark: boolean;
  onNavigate: (input: string) => void;
  onSuggest: (input: string) => Promise<Suggestion[]>;
  bookmarked: boolean;
  onToggleBookmark: () => void;
}) {
  const [value, setValue] = useState("");
  const [focused, setFocused] = useState(false);
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const debounceRef = useRef(0);

  const display = props.tab ? displayUrl(props.tab.url) : "";

  // Keep the field in sync with the tab when not editing.
  useEffect(() => {
    if (!focused) setValue(display);
  }, [display, focused]);

  // Focus the omnibox from anywhere (Ctrl+L / accelerator event).
  useEffect(() => {
    const focus = () => {
      inputRef.current?.focus();
      inputRef.current?.select();
    };
    const un = listen<string>("accelerator", (e) => {
      if (e.payload === "accel-focus-omnibox") focus();
    });
    return () => { void un.then((f) => f()); };
  }, []);

  // Debounced suggestions; never fire for empty input.
  useEffect(() => {
    window.clearTimeout(debounceRef.current);
    if (!focused || !value.trim()) {
      setSuggestions([]);
      setSelected(0);
      return;
    }
    debounceRef.current = window.setTimeout(async () => {
      setSuggestions(await props.onSuggest(value));
      setSelected(0);
    }, 90);
    return () => window.clearTimeout(debounceRef.current);
  }, [value, focused, props]);

  function commit(input: string) {
    if (!input.trim()) return;
    setFocused(false);
    setSuggestions([]);
    inputRef.current?.blur();
    props.onNavigate(input);
  }

  function onKeyDown(e: ReactKeyboardEvent<HTMLInputElement>) {
    if (e.key === "Escape") {
      inputRef.current?.blur();
      setFocused(false);
      setSuggestions([]);
      return;
    }
    if (suggestions.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((s) => (s + 1) % suggestions.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => (s - 1 + suggestions.length) % suggestions.length);
    } else if (e.key === "Enter" && selected > 0) {
      e.preventDefault();
      const s = suggestions[selected];
      commit(s.url);
    }
  }

  const showPanel = focused && suggestions.length > 0;

  return (
    <div className="relative min-w-0 flex-1">
      <div
        className={`flex h-9 items-center gap-2 rounded-full border px-3.5 transition-colors
          ${props.dark
            ? "border-zinc-700 bg-zinc-800 focus-within:border-sky-500"
            : "border-zinc-300 bg-zinc-100 focus-within:border-sky-500"}`}
      >
        {props.tab && isSecureUrl(props.tab.url) ? (
          <ShieldCheck size={14} className="shrink-0 text-emerald-500" />
        ) : props.tab && !isHomeUrl(props.tab.url) ? (
          <Globe size={14} className="shrink-0 opacity-50" />
        ) : (
          <Search size={14} className="shrink-0 opacity-50" />
        )}
        <input
          ref={inputRef}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onFocus={(e) => { setFocused(true); e.currentTarget.select(); }}
          onBlur={() => { setFocused(false); setSuggestions([]); }}
          onKeyDown={onKeyDown}
          placeholder={`Search ${props.engineName} or enter address`}
          spellCheck={false}
          autoComplete="off"
          aria-label="Address and search bar"
          className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:opacity-50"
        />
        {props.tab && !isHomeUrl(props.tab.url) && (
          <button
            type="button"
            title={props.bookmarked ? "Remove bookmark" : "Bookmark this page"}
            onClick={props.onToggleBookmark}
            className="shrink-0 rounded p-0.5 opacity-60 transition-opacity hover:opacity-100"
          >
            {props.bookmarked
              ? <Star size={14} className="fill-amber-400 text-amber-400" />
              : <StarOff size={14} />}
          </button>
        )}
      </div>

      {showPanel && (
        <div
          onMouseDown={(e) => e.preventDefault()} // keep input focus while clicking
          className={`absolute inset-x-0 top-11 z-40 overflow-hidden rounded-xl border shadow-2xl
            ${props.dark ? "border-zinc-700 bg-zinc-800" : "border-zinc-200 bg-white"}`}
        >
          {suggestions.map((s, i) => {
            const icon =
              s.kind === "openTab" ? <Square size={13} className="text-sky-400" />
              : s.kind === "bookmark" ? <BookmarkIcon size={13} className="text-amber-400" />
              : s.kind === "history" ? <Clock size={13} className="opacity-50" />
              : <Search size={13} className="text-emerald-400" />;
            const label =
              s.kind === "search" ? s.query : s.title || s.url;
            const second =
              s.kind === "search" ? `${props.engineName} search` : displayUrl(s.url);
            return (
              <button
                key={`${s.kind}-${s.url}-${i}`}
                type="button"
                onMouseEnter={() => setSelected(i)}
                onClick={() => commit(s.url)}
                className={`flex w-full items-center gap-2.5 px-3.5 py-2 text-left text-sm
                  ${i === selected ? (props.dark ? "bg-zinc-700/70" : "bg-zinc-100") : ""}
                  ${i > 0 ? "border-t border-black/5 dark:border-white/5" : ""}`}
              >
                <span className="shrink-0">{icon}</span>
                <span className="min-w-0 flex-1 truncate">{label}</span>
                <span className="shrink-0 text-xs opacity-40">{second}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Side panels                                                         */
/* ------------------------------------------------------------------ */

type PanelKind = "history" | "bookmarks" | "downloads" | "settings";

function Panel(props: {
  kind: PanelKind;
  dark: boolean;
  onClose: () => void;
  history: HistoryEntry[];
  bookmarks: Bookmark[];
  downloads: DownloadItem[];
  onNavigate: (url: string) => void;
  onRemoveBookmark: (id: string) => void;
  onClearHistory: () => void;
  settings: ReturnType<typeof useBrowser>["settings"];
  engines: ReturnType<typeof useBrowser>["engines"];
  onSaveSettings: (s: NonNullable<ReturnType<typeof useBrowser>["settings"]>) => void;
}) {
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState(props.settings ?? null);

  useEffect(() => { setDraft(props.settings); }, [props.settings]);

  const filteredHistory = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return props.history;
    return props.history.filter(
      (h) => h.url.toLowerCase().includes(q) || (h.title ?? "").toLowerCase().includes(q),
    );
  }, [props.history, query]);

  const title =
    props.kind === "history" ? "History"
    : props.kind === "bookmarks" ? "Bookmarks"
    : props.kind === "downloads" ? "Downloads"
    : "Settings";

  return (
    <aside
      className={`flex w-80 shrink-0 flex-col border-l
        ${props.dark ? "border-zinc-800 bg-zinc-900" : "border-zinc-200 bg-white"}`}
    >
      <header className="flex items-center justify-between border-b px-3.5 py-2.5"
        style={{ borderColor: props.dark ? "#27272a" : "#e4e4e7" }}>
        <h2 className="text-sm font-semibold">{title}</h2>
        <IconButton title="Close panel" onClick={props.onClose}><X size={15} /></IconButton>
      </header>

      {props.kind === "history" && (
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="flex items-center gap-2 border-b px-3.5 py-2"
            style={{ borderColor: props.dark ? "#27272a" : "#e4e4e7" }}>
            <Search size={13} className="opacity-40" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search history"
              className="min-w-0 flex-1 bg-transparent text-sm outline-none"
              spellCheck={false}
            />
            <button
              type="button"
              title="Clear all history"
              onClick={props.onClearHistory}
              className="flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-red-400 hover:bg-red-500/10"
            >
              <Trash2 size={12} /> Clear
            </button>
          </div>
          <div className="min-h-0 flex-1 overflow-auto p-2">
            {filteredHistory.length === 0 && (
              <p className="px-2 py-6 text-center text-sm opacity-50">No history yet.</p>
            )}
            {filteredHistory.map((h) => (
              <button
                key={h.id}
                type="button"
                onClick={() => props.onNavigate(h.url)}
                className="w-full rounded-lg px-2.5 py-2 text-left hover:bg-black/5 dark:hover:bg-white/5"
              >
                <div className="truncate text-sm">{h.title || h.url}</div>
                <div className="truncate text-xs opacity-50">{displayUrl(h.url)}</div>
              </button>
            ))}
          </div>
        </div>
      )}

      {props.kind === "bookmarks" && (
        <div className="min-h-0 flex-1 overflow-auto p-2">
          {props.bookmarks.length === 0 && (
            <p className="px-2 py-6 text-center text-sm opacity-50">
              No bookmarks yet. Star a page to keep it here.
            </p>
          )}
          {props.bookmarks.map((b) => (
            <div key={b.id} className="group flex items-center gap-1 rounded-lg hover:bg-black/5 dark:hover:bg-white/5">
              <button
                type="button"
                onClick={() => props.onNavigate(b.url)}
                className="min-w-0 flex-1 px-2.5 py-2 text-left"
              >
                <div className="truncate text-sm">{b.title}</div>
                <div className="truncate text-xs opacity-50">{displayUrl(b.url)}</div>
              </button>
              <button
                type="button"
                title="Remove bookmark"
                onClick={() => props.onRemoveBookmark(b.id)}
                className="mr-1 rounded p-1.5 opacity-0 transition-opacity hover:text-red-400 group-hover:opacity-60"
              >
                <Trash2 size={13} />
              </button>
            </div>
          ))}
        </div>
      )}

      {props.kind === "downloads" && (
        <div className="min-h-0 flex-1 overflow-auto p-2">
          {props.downloads.length === 0 && (
            <p className="px-2 py-6 text-center text-sm opacity-50">No downloads yet.</p>
          )}
          {props.downloads.map((d) => (
            <div key={d.id} className="rounded-lg px-2.5 py-2">
              <div className="flex items-center gap-2">
                <Download size={13} className="shrink-0 opacity-50" />
                <span className="min-w-0 flex-1 truncate text-sm">{d.filename}</span>
                <span
                  className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium
                    ${d.status === "completed" ? "bg-emerald-500/15 text-emerald-400"
                      : d.status === "failed" ? "bg-red-500/15 text-red-400"
                      : "bg-sky-500/15 text-sky-400"}`}
                >
                  {d.status}
                </span>
              </div>
              <div className="mt-0.5 truncate text-xs opacity-40">{d.path}</div>
            </div>
          ))}
        </div>
      )}

      {props.kind === "settings" && draft && (
        <div className="min-h-0 flex-1 space-y-5 overflow-auto p-4 text-sm">
          <section>
            <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider opacity-50">Appearance</h3>
            <label className="flex items-center justify-between py-1.5">
              Theme
              <select
                value={draft.theme}
                onChange={(e) => setDraft({ ...draft, theme: e.target.value })}
                className={`rounded-lg border px-2 py-1.5 text-sm outline-none
                  ${props.dark ? "border-zinc-700 bg-zinc-800" : "border-zinc-300 bg-zinc-100"}`}
              >
                <option value="dark">Dark</option>
                <option value="light">Light</option>
              </select>
            </label>
            <label className="flex items-center justify-between py-1.5">
              Show bookmarks bar
              <input
                type="checkbox"
                checked={draft.showBookmarksBar}
                onChange={(e) => setDraft({ ...draft, showBookmarksBar: e.target.checked })}
                className="h-4 w-4 accent-sky-500"
              />
            </label>
          </section>

          <section>
            <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider opacity-50">Search</h3>
            <label className="flex items-center justify-between py-1.5">
              Default engine
              <select
                value={draft.defaultEngine}
                onChange={(e) => setDraft({ ...draft, defaultEngine: e.target.value })}
                className={`rounded-lg border px-2 py-1.5 text-sm outline-none
                  ${props.dark ? "border-zinc-700 bg-zinc-800" : "border-zinc-300 bg-zinc-100"}`}
              >
                {props.engines.map((e) => (
                  <option key={e.name} value={e.name}>{e.name}</option>
                ))}
              </select>
            </label>
            <p className="mt-1.5 text-xs opacity-40">
              Tip: type an engine keyword first (e.g. “g cats”) to search with another engine.
            </p>
          </section>

          <section>
            <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider opacity-50">Startup</h3>
            <label className="flex items-center justify-between py-1.5">
              Homepage
              <input
                value={draft.homepage}
                onChange={(e) => setDraft({ ...draft, homepage: e.target.value })}
                spellCheck={false}
                className={`w-40 rounded-lg border px-2 py-1.5 text-sm outline-none
                  ${props.dark ? "border-zinc-700 bg-zinc-800" : "border-zinc-300 bg-zinc-100"}`}
              />
            </label>
            <label className="flex items-center justify-between py-1.5">
              Block pop-ups
              <input
                type="checkbox"
                checked={draft.blockPopups}
                onChange={(e) => setDraft({ ...draft, blockPopups: e.target.checked })}
                className="h-4 w-4 accent-sky-500"
              />
            </label>
          </section>

          <button
            type="button"
            onClick={() => props.onSaveSettings(draft)}
            className="w-full rounded-lg bg-sky-600 py-2 font-medium text-white transition-colors hover:bg-sky-500"
          >
            Save settings
          </button>

          <p className="text-center text-[11px] leading-relaxed opacity-35">
            RustBrowzer v0.2.0 · Rust + Tauri · WebKitGTK
            <br />
            Only the active tab is kept in memory; switching reloads it.
          </p>
        </div>
      )}
    </aside>
  );
}

/* ------------------------------------------------------------------ */
/* Tab context menu                                                    */
/* ------------------------------------------------------------------ */

function ContextMenu(props: {
  x: number; y: number; tab: Tab; dark: boolean;
  onClose: () => void;
  onReload: () => void; onCloseTab: () => void;
  onPin: () => void; onMute: () => void; onCloseOthers: () => void;
}) {
  useEffect(() => {
    const close = () => props.onClose();
    window.addEventListener("click", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("blur", close);
    };
  }, [props]);

  const item = "flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-sm hover:bg-black/5 dark:hover:bg-white/10";

  return (
    <div
      style={{ position: "fixed", left: props.x, top: props.y, zIndex: 60 } as CSSProperties}
      onClick={(e) => e.stopPropagation()}
      className={`w-52 overflow-hidden rounded-xl border p-1 shadow-2xl
        ${props.dark ? "border-zinc-700 bg-zinc-800 text-zinc-100" : "border-zinc-200 bg-white text-zinc-900"}`}
    >
      <button type="button" className={item} onClick={() => { props.onReload(); props.onClose(); }}>
        <RotateCw size={14} /> Reload
      </button>
      <button type="button" className={item} onClick={() => { props.onPin(); props.onClose(); }}>
        <Pin size={14} /> {props.tab.pinned ? "Unpin tab" : "Pin tab"}
      </button>
      <button type="button" className={item} onClick={() => { props.onMute(); props.onClose(); }}>
        {props.tab.muted ? <Volume2 size={14} /> : <VolumeX size={14} />}
        {props.tab.muted ? "Unmute tab" : "Mute tab"}
      </button>
      <div className="my-1 border-t border-black/10 dark:border-white/10" />
      <button type="button" className={`${item} text-red-400`} onClick={() => { props.onCloseOthers(); props.onClose(); }}>
        <X size={14} /> Close other tabs
      </button>
      <button type="button" className={`${item} text-red-400`} onClick={() => { props.onCloseTab(); props.onClose(); }}>
        <X size={14} /> Close tab
      </button>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* App                                                                 */
/* ------------------------------------------------------------------ */

export default function App() {
  const browser = useBrowser();
  const { tabs, activeId, activeTab, settings, engines, actions } = browser;
  const [panel, setPanel] = useState<PanelKind | null>(null);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);
  const [downloads, setDownloads] = useState<DownloadItem[]>([]);
  const [bookmarked, setBookmarked] = useState(false);
  const [toast, setToast] = useState<{ message: string; kind: "success" | "error" } | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; tab: Tab } | null>(null);
  const closedTabsRef = useRef<{ url: string; isPrivate: boolean }[]>([]);

  const dark = settings?.theme !== "light";

  const showToast = (message: string, kind: "success" | "error" = "success") => {
    setToast({ message, kind });
    window.setTimeout(() => setToast(null), 2600);
  };

  // Track recently closed tabs for Ctrl+Shift+T (mirrors backend state).
  useEffect(() => {
    const onTabs = (next: Tab[]) => {
      const urls = new Set(next.map((t) => t.id));
      // Nothing to do here on add; closure below uses closeTab wrapper.
      void urls;
    };
    onTabs(tabs);
  }, [tabs]);

  async function handleCloseTab(id: string) {
    const tab = tabs.find((t) => t.id === id);
    if (tab) {
      closedTabsRef.current.unshift({ url: tab.url, isPrivate: tab.isPrivate });
      closedTabsRef.current = closedTabsRef.current.slice(0, 10);
    }
    await actions.closeTab(id);
  }

  async function handleReopen() {
    const last = closedTabsRef.current.shift();
    await actions.reopenTab(last?.url, last?.isPrivate);
  }

  // Panel data loading (only when the panel is open).
  useEffect(() => {
    if (panel === "history") void actions.listHistory(200).then(setHistory);
    if (panel === "bookmarks") void actions.listBookmarks().then(setBookmarks);
    if (panel === "downloads") void actions.listDownloads().then(setDownloads);
  }, [panel, actions]);

  // Keep downloads fresh while the panel is open.
  useEffect(() => {
    if (panel !== "downloads") return;
    const un = listen("downloads-changed", () => {
      void actions.listDownloads().then(setDownloads);
    });
    return () => { void un.then((f) => f()); };
  }, [panel, actions]);

  // Bookmark star state follows the active tab.
  useEffect(() => {
    if (!activeTab || isHomeUrl(activeTab.url)) {
      setBookmarked(false);
      return;
    }
    void actions.isBookmarked(activeTab.url).then(setBookmarked);
  }, [activeTab?.url, actions]);

  async function toggleBookmark() {
    if (!activeTab) return;
    if (bookmarked) {
      const b = bookmarks.find((b) => b.url === activeTab.url);
      if (b) await actions.removeBookmark(b.id);
      setBookmarked(false);
      showToast("Bookmark removed");
    } else {
      const ok = await actions.addBookmark(activeTab.title || activeTab.url, activeTab.url);
      setBookmarked(ok);
      showToast(ok ? "Bookmark added" : "Could not bookmark this page", ok ? "success" : "error");
      if (ok && panel === "bookmarks") {
        void actions.listBookmarks().then(setBookmarks);
      }
    }
  }

  // Keyboard shortcuts (work when the UI webview has focus; GTK accelerators
  // cover the case where web content has focus).
  useEffect(() => {
    const onKey = async (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const key = e.key.toLowerCase();
      if (e.shiftKey && key === "t") { e.preventDefault(); await handleReopen(); return; }
      switch (key) {
        case "t": e.preventDefault(); await actions.newTab(); break;
        case "w": e.preventDefault(); if (activeId) await handleCloseTab(activeId); break;
        case "l": e.preventDefault();
          (document.querySelector('input[aria-label="Address and search bar"]') as HTMLInputElement | null)?.focus();
          break;
        case "r": e.preventDefault(); await actions.reload(); break;
        case "b": e.preventDefault(); setPanel((p) => (p === "bookmarks" ? null : "bookmarks")); break;
        case "h": e.preventDefault(); setPanel((p) => (p === "history" ? null : "history")); break;
        case "j": e.preventDefault(); setPanel((p) => (p === "downloads" ? null : "downloads")); break;
        case "tab": {
          e.preventDefault();
          if (tabs.length < 2) break;
          const idx = tabs.findIndex((t) => t.id === activeId);
          const next = tabs[(idx + (e.shiftKey ? -1 : 1) + tabs.length) % tabs.length];
          await actions.selectTab(next.id);
          break;
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [actions, activeId, tabs]);

  // Backend accelerator routing (fires when web content had focus).
  useEffect(() => {
    const un = listen<string>("accelerator", async (e) => {
      switch (e.payload) {
        case "accel-new-tab": await actions.newTab(); break;
        case "accel-close-tab": if (activeId) await handleCloseTab(activeId); break;
        case "accel-reopen-tab": await handleReopen(); break;
        case "accel-reload": await actions.reload(); break;
        case "accel-toggle-bookmarks": setPanel((p) => (p === "bookmarks" ? null : "bookmarks")); break;
        case "accel-toggle-history": setPanel((p) => (p === "history" ? null : "history")); break;
        case "accel-toggle-downloads": setPanel((p) => (p === "downloads" ? null : "downloads")); break;
      }
    });
    return () => { void un.then((f) => f()); };
  }, [actions, activeId]);

  const engineName = useMemo(() => {
    const s = settings;
    return engines.find((e) => e.name === s?.defaultEngine)?.name ?? engines[0]?.name ?? "the web";
  }, [settings, engines]);

  const chrome = dark ? "border-zinc-800 bg-zinc-900" : "border-zinc-200 bg-zinc-50";

  if (!settings) {
    // Skeleton while the snapshot loads.
    return (
      <div className={`flex h-full items-center justify-center ${dark ? "bg-zinc-950 text-zinc-400" : "bg-zinc-100 text-zinc-500"}`}>
        <div className="flex items-center gap-2.5 text-sm">
          <span className="h-4 w-4 animate-pulse rounded-full bg-sky-500" />
          Starting…
        </div>
      </div>
    );
  }

  return (
    <div className={`flex h-full flex-col ${dark ? "bg-zinc-950 text-zinc-100" : "bg-zinc-100 text-zinc-900"}`}>
      {/* ---- tab strip ---- */}
      <div className={`flex items-end gap-0.5 border-b px-2 pt-1.5 ${chrome}`}>
        <div className="flex min-w-0 flex-1 items-end gap-0.5 overflow-x-auto" style={{ scrollbarWidth: "none" }}>
          {tabs.map((t) => (
            <TabChip
              key={t.id}
              tab={t}
              active={t.id === activeId}
              dark={dark}
              onSelect={() => void actions.selectTab(t.id)}
              onClose={() => void handleCloseTab(t.id)}
              onPin={() => void actions.togglePin(t.id)}
              onMute={() => void actions.toggleMute(t.id)}
              onContextMenu={(e) => { e.preventDefault(); setMenu({ x: e.clientX, y: e.clientY, tab: t }); }}
            />
          ))}
          <button
            type="button"
            title="New tab (Ctrl+T)"
            onClick={() => void actions.newTab()}
            className="mb-0.5 ml-1 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg opacity-60 transition-opacity hover:opacity-100"
          >
            <Plus size={15} />
          </button>
        </div>
        <button
          type="button"
          title="New private tab"
          onClick={() => void actions.newTab(undefined, true)}
          className="mb-1 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg opacity-60 transition-opacity hover:opacity-100"
        >
          <EyeOff size={14} />
        </button>
      </div>

      {/* ---- toolbar ---- */}
      <div className={`flex items-center gap-1 border-b px-2 py-1.5 ${chrome}`}>
        <IconButton title="Back (Alt+←)" disabled={!activeTab?.canGoBack} onClick={() => void actions.goBack()}>
          <ArrowLeft size={16} />
        </IconButton>
        <IconButton title="Forward (Alt+→)" disabled={!activeTab?.canGoForward} onClick={() => void actions.goForward()}>
          <ArrowRight size={16} />
        </IconButton>
        {activeTab?.loading ? (
          <IconButton title="Stop loading" onClick={() => void actions.stop()}><Square size={14} /></IconButton>
        ) : (
          <IconButton title="Reload (Ctrl+R)" disabled={!activeTab || isHomeUrl(activeTab.url)} onClick={() => void actions.reload()}>
            <RotateCw size={15} />
          </IconButton>
        )}
        <IconButton title="Home" onClick={() => void actions.navigate("browser://home")}>
          <Home size={15} />
        </IconButton>

        <Omnibox
          tab={activeTab}
          engineName={engineName}
          dark={dark}
          onNavigate={(input) => void actions.navigate(input)}
          onSuggest={actions.suggest}
          bookmarked={bookmarked}
          onToggleBookmark={() => void toggleBookmark()}
        />

        <IconButton title="Bookmarks (Ctrl+B)" active={panel === "bookmarks"}
          onClick={() => setPanel((p) => (p === "bookmarks" ? null : "bookmarks"))}>
          <Star size={15} />
        </IconButton>
        <IconButton title="History (Ctrl+H)" active={panel === "history"}
          onClick={() => setPanel((p) => (p === "history" ? null : "history"))}>
          <HistoryIcon size={15} />
        </IconButton>
        <IconButton title="Downloads (Ctrl+J)" active={panel === "downloads"}
          onClick={() => setPanel((p) => (p === "downloads" ? null : "downloads"))}>
          <Download size={15} />
        </IconButton>
        <IconButton
          title={dark ? "Light theme" : "Dark theme"}
          onClick={() => void actions.saveSettings({ ...settings, theme: dark ? "light" : "dark" })}
        >
          {dark ? <Sun size={15} /> : <Moon size={15} />}
        </IconButton>
        <IconButton title="Settings" active={panel === "settings"}
          onClick={() => setPanel((p) => (p === "settings" ? null : "settings"))}>
          <SettingsIcon size={15} />
        </IconButton>
      </div>

      {/* ---- bookmarks bar ---- */}
      {settings.showBookmarksBar && bookmarks.length > 0 && (
        <div className={`flex items-center gap-1 overflow-x-auto border-b px-2 py-1 text-xs ${chrome}`}
          style={{ scrollbarWidth: "none" }}>
          <button type="button" onClick={() => void actions.listBookmarks().then(setBookmarks)}
            className="shrink-0 rounded px-1.5 py-0.5 opacity-50 hover:opacity-100" title="Refresh">
            <BookmarkIcon size={12} />
          </button>
          {bookmarks.slice(0, 12).map((b) => (
            <button
              key={b.id}
              type="button"
              title={b.url}
              onClick={() => void actions.navigate(b.url)}
              className="flex max-w-[160px] shrink-0 items-center gap-1.5 rounded-md px-2 py-1 transition-colors hover:bg-black/10 dark:hover:bg-white/10"
            >
              <ExternalLink size={11} className="shrink-0 opacity-40" />
              <span className="truncate">{b.title}</span>
            </button>
          ))}
        </div>
      )}

      {/* ---- content + panel ---- */}
      <div className="flex min-h-0 flex-1">
        <div ref={browser.viewportRef} className="relative min-w-0 flex-1">
          {(!activeTab || isHomeUrl(activeTab.url)) ? (
            <HomePage
              dark={dark}
              engineName={engineName}
              isPrivate={!!activeTab?.isPrivate}
              onNavigate={(input) => void actions.navigate(input)}
            />
          ) : (
            /* The native webview renders on top of this host; this element
               only sets the background while switching. */
            <div className={`absolute inset-0 ${dark ? "bg-zinc-900" : "bg-white"}`} />
          )}
        </div>

        {panel && (
          <Panel
            kind={panel}
            dark={dark}
            onClose={() => setPanel(null)}
            history={history}
            bookmarks={bookmarks}
            downloads={downloads}
            onNavigate={(url) => void actions.navigate(url)}
            onRemoveBookmark={async (id) => {
              await actions.removeBookmark(id);
              setBookmarks(await actions.listBookmarks());
              showToast("Bookmark removed");
            }}
            onClearHistory={async () => {
              await actions.clearHistory();
              setHistory([]);
              showToast("History cleared");
            }}
            settings={settings}
            engines={engines}
            onSaveSettings={async (s) => {
              const ok = await actions.saveSettings(s);
              showToast(ok ? "Settings saved" : "Could not save settings", ok ? "success" : "error");
            }}
          />
        )}
      </div>

      {menu && (
        <ContextMenu
          x={menu.x} y={menu.y} tab={menu.tab} dark={dark}
          onClose={() => setMenu(null)}
          onReload={() => void actions.reload(menu.tab.id)}
          onCloseTab={() => void handleCloseTab(menu.tab.id)}
          onPin={() => void actions.togglePin(menu.tab.id)}
          onMute={() => void actions.toggleMute(menu.tab.id)}
          onCloseOthers={async () => {
            for (const t of tabs.filter((t) => t.id !== menu.tab.id)) {
              await actions.closeTab(t.id);
            }
          }}
        />
      )}

      <Toasts toast={toast} />
    </div>
  );
}
