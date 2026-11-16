import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowLeft, ArrowRight, RotateCw, Home, Star, Puzzle, Download, UserCircle2,
  Settings, Plus, X, Shield, Search, History, Moon, Sun, Lock, Globe, Pin, VolumeX
} from "lucide-react";

type Tab = {
  id: string; title: string; url: string; loading: boolean; pinned: boolean; muted: boolean;
  isPrivate: boolean; canGoBack: boolean; canGoForward: boolean;
};
type TabState = { tabs: Tab[]; activeTabId: string | null; closedTabs: Tab[] };
type SettingsT = { theme: string; searchEngine: string; homepage: string; showBookmarksBar: boolean };

function isSecure(url: string) { return url.startsWith("https://") || url.startsWith("about:") || url.startsWith("tauri:"); }

export default function App() {
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [settings, setSettings] = useState<SettingsT>({ theme: "dark", searchEngine: "https://www.google.com/search?q=", homepage: "https://example.com", showBookmarksBar: true });
  const [address, setAddress] = useState("");
  const [panel, setPanel] = useState<"none"|"history"|"bookmarks"|"downloads"|"settings">("none");
  const [history, setHistory] = useState<any[]>([]);
  const [bookmarks, setBookmarks] = useState<any[]>([]);
  const [refreshing, setRefreshing] = useState(false);
  const viewportRef = useRef<HTMLDivElement>(null);
  const activeTab = useMemo(() => tabs.find(t => t.id === activeId) || null, [tabs, activeId]);

  async function refresh() {
    setRefreshing(true);
    try {
      const snap = await invoke<{tabs: TabState, settings: SettingsT}>("get_snapshot");
      setTabs(snap.tabs.tabs);
      setActiveId(snap.tabs.activeTabId);
      setSettings(snap.settings as any);
      const a = snap.tabs.tabs.find(t => t.id === snap.tabs.activeTabId);
      if (a) setAddress(a.url);
    } catch (e) {
      console.error("Failed to refresh:", e);
      showToast("Failed to refresh", "error");
    } finally {
      setRefreshing(false);
    }
  }

  useEffect(() => { refresh(); }, []);
  useEffect(() => { if (activeTab) setAddress(activeTab.url); }, [activeId]);

  async function getViewport() {
    const el = viewportRef.current; if (!el) return null;
    const r = el.getBoundingClientRect();
    // logical coords relative to window webview
    return { x: r.left, y: r.top, width: r.width, height: r.height };
  }

  // Keep tab metadata for many tabs, but only keep a live webview for the active tab (memory optimized).
  // Inactive tabs are discarded and reloaded on activation (state within page is not preserved across discard).
  async function showActiveContent(tab?: Tab | null, tabList?: Tab[]) {
    const t = tab ?? activeTab;
    const list = tabList ?? tabs;
    const vp = await getViewport();
    if (!t || !vp || vp.width < 50 || vp.height < 50) return;
    await Promise.all(list.filter(o => o.id !== t.id).map(o => invoke("close_content", { tabId: o.id }).catch(()=>{})));
    if (!t.url || t.url === "about:blank") {
      await invoke("close_content", { tabId: t.id }).catch(()=>{});
      return;
    }
    await invoke("open_content", { tabId: t.id, url: t.url, viewport: vp }).catch((e)=>console.error(e));
  }

  useEffect(() => {
    showActiveContent();
  }, [activeId]);

  const resizeRef = useRef<number>(0);

  const debouncedShowContent = useMemo(() => {
    return (tab?: Tab | null, tabList?: Tab[]) => {
      if (resizeRef.current) {
        window.cancelAnimationFrame(resizeRef.current);
      }
      resizeRef.current = window.requestAnimationFrame(() => {
        showActiveContent(tab, tabList);
      });
    };
  }, [activeId, tabs]);

  useEffect(() => {
    const onResize = () => {
      debouncedShowContent(activeTab, tabs);
    };
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
      if (resizeRef.current) {
        window.cancelAnimationFrame(resizeRef.current);
      }
    };
  }, [activeTab, tabs, debouncedShowContent]);

  // Update tab loading state when navigation completes
  useEffect(() => {
    if (!activeTab?.loading) return;

    const timer = setTimeout(async () => {
      try {
        // Simulate page load completion
        const updatedTab = await invoke<Tab>("get_tab", { id: activeTab.id });
        setTabs(ts => ts.map(t => t.id === activeTab.id ? updatedTab : t));
      } catch (e) {
        console.error("Failed to update tab state:", e);
      }
    }, 500);

    return () => clearTimeout(timer);
  }, [activeTab?.loading, activeTab?.id]);

  async function navigateTo(raw: string) {
    if (!activeTab) return;
    try {
      const url = await invoke<string>("resolve_url", { input: raw });
      setTabs(ts => ts.map(t => t.id === activeTab.id ? { ...t, url, loading: true, title: url } : t));
      setAddress(url);
      await invoke("navigate_tab", { payload: { id: activeTab.id, url, title: url } }).catch(e => console.error("Navigation error:", e));
      const vp = await getViewport();
      if (vp && url && url !== "about:blank") {
        await invoke("open_content", { tabId: activeTab.id, url, viewport: vp }).catch(e => console.error("Failed to open content:", e));
      }
    } catch (e) {
      console.error("Navigation failed:", e);
      // Show error message to user
      showToast("Failed to navigate: " + (e instanceof Error ? e.message : String(e)), "error");
    }
  }

  async function addTab(url?: string, isPrivate=false) {
    const t = await invoke<Tab>("new_tab", { url: url ?? settings.homepage, isPrivate });
    setTabs(ts => {
      const next = [...ts, t];
      setTimeout(()=>showActiveContent(t, next), 30);
      return next;
    });
    setActiveId(t.id);
    setAddress(t.url);
  }

  async function toggleTabPin(id: string, e: React.MouseEvent) {
    e.stopPropagation();
    await invoke("toggle_tab_pin", { id });
    await refresh();
  }

  async function toggleTabMute(id: string, e: React.MouseEvent) {
    e.stopPropagation();
    await invoke("toggle_tab_mute", { id });
    await refresh();
  }

  async function removeTab(id: string) {
    await invoke("close_content", { tabId: id }).catch(()=>{});
    await invoke<string | null>("close_tab", { id });
    await refresh();
    showToast("Tab closed", "success");
  }

  async function selectTab(id: string) {
    if (id === activeId) return;
    setActiveId(id);
    await invoke("set_active_tab", { id });
  }

  async function loadHistory() { setHistory(await invoke("list_history", { limit: 200 })); }
  async function loadBookmarks() { setBookmarks(await invoke("list_bookmarks")); }
  async function addBookmarkCurrent() {
    if (!activeTab) return;
    try {
      await invoke("add_bookmark", { title: activeTab.title || activeTab.url, url: activeTab.url, folder: null });
      showToast("Bookmark added", "success");
      if (panel==="bookmarks") loadBookmarks();
    } catch (e) {
      showToast("Failed to add bookmark", "error");
    }
  }

  async function toggleTheme() {
    const next = { ...settings, theme: settings.theme === "dark" ? "light" : "dark" };
    setSettings(next); await invoke("save_settings", { settings: next });
  }

  const dark = settings.theme !== "light";
  const bg = dark ? "bg-zinc-950 text-zinc-100" : "bg-zinc-100 text-zinc-900";
  const chrome = dark ? "bg-zinc-900/95 border-zinc-800" : "bg-white border-zinc-200";
  const tabIdle = dark ? "bg-zinc-800/60 hover:bg-zinc-800 text-zinc-300" : "bg-zinc-200/70 hover:bg-zinc-200 text-zinc-700";
  const tabActive = dark ? "bg-zinc-950 text-white shadow" : "bg-white text-zinc-900 shadow";

  // Toast notification system
  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);

  const showToast = (message: string, type: 'success' | 'error') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        switch (e.key.toLowerCase()) {
          case 't':
            e.preventDefault();
            await addTab();
            break;
          case 'w':
            e.preventDefault();
            if (activeId) await removeTab(activeId);
            break;
          case 'l':
            e.preventDefault();
            const addressInput = document.querySelector('input[type="text"]') as HTMLInputElement;
            if (addressInput) {
              addressInput.focus();
              addressInput.select();
            }
            break;
          case 'b':
            e.preventDefault();
            setPanel(p => p === "bookmarks" ? "none" : "bookmarks");
            loadBookmarks();
            break;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [activeId]);

  return (
    <div className={`h-full flex flex-col ${bg}`}>
      {/* Tab bar */}
      <div className={`flex items-center gap-1 px-2 pt-2 border-b ${chrome}`}>
        <div className="flex-1 flex items-center gap-1 overflow-x-auto">
          {tabs.map(t => (
            <div key={t.id} onClick={()=>selectTab(t.id)} title={t.title || t.url}
              className={`group flex items-center gap-2 px-3 py-1.5 rounded-t-lg min-w-[110px] max-w-[240px] shrink-0 cursor-pointer border border-b-0 ${t.id===activeId?tabActive:tabIdle} ${dark?"border-zinc-700":"border-zinc-300"}`}>
              {isSecure(t.url) ? <Lock size={13} className="text-emerald-400 shrink-0"/> : <Globe size={13} className="shrink-0 opacity-60"/>}
              {t.pinned && <Pin size={13} className="text-amber-400 shrink-0"/>}
              {t.muted && <VolumeX size={13} className="text-red-400 shrink-0"/>}
              <span className="truncate text-xs flex-1">{t.isPrivate?"🕶 ":""}{t.loading ? <span className="inline-block w-1.5 h-1.5 bg-blue-400 rounded-full animate-pulse mr-1"/> : ""}{t.title || t.url || "New Tab"}</span>
              <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
                <button onClick={(e)=>{e.stopPropagation(); toggleTabPin(t.id, e);}} className="hover:bg-zinc-500/20 rounded p-0.5" title="Pin tab"><Pin size={12}/></button>
                <button onClick={(e)=>{e.stopPropagation(); toggleTabMute(t.id, e);}} className="hover:bg-zinc-500/20 rounded p-0.5" title="Mute tab"><VolumeX size={12}/></button>
                <button onClick={(e)=>{e.stopPropagation(); removeTab(t.id);}} className="hover:bg-zinc-500/20 rounded p-0.5"><X size={12}/></button>
              </div>
            </div>
          ))}
          <button onClick={()=>addTab()} className="p-1.5 rounded hover:bg-zinc-500/20" title="New tab (Ctrl+T)"><Plus size={16}/></button>
        </div>
        <div className="text-[11px] opacity-60 px-2 whitespace-nowrap" title="Open tabs (active tab rendered live; inactive tabs are suspended to save memory)">{tabs.length} tabs</div>
      </div>

      {/* Toolbar */}
      <div className={`flex items-center gap-1.5 px-2 py-2 border-b ${chrome}`}>
        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="Back" onClick={()=>{ /* engine history wiring v1 */ }}><ArrowLeft size={16}/></button>
        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="Forward"><ArrowRight size={16}/></button>
        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="Reload" onClick={()=>activeTab && navigateTo(activeTab.url)}><RotateCw size={16}/></button>
        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="Home" onClick={()=>navigateTo(settings.homepage)}><Home size={16}/></button>

        <form className="flex-1 flex items-center" onSubmit={(e)=>{e.preventDefault(); navigateTo(address);}}>
          <div className={`flex items-center gap-2 w-full px-3 py-1.5 rounded-full border ${dark?"bg-zinc-800 border-zinc-700":"bg-zinc-100 border-zinc-300"}`}>
            {activeTab && isSecure(activeTab.url) ? <Shield size={14} className="text-emerald-400"/> : <Search size={14} className="opacity-60"/>}
            {refreshing && <RotateCw size={14} className="animate-spin text-blue-400"/>}
            <input
              value={address}
              onChange={e=>setAddress(e.target.value)}
              onKeyDown={e => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  navigateTo(address);
                }
              }}
              className="flex-1 bg-transparent outline-none text-sm"
              placeholder="Search or enter address"
              spellCheck={false}
              autoComplete="off"
            />
            <button type="button" onClick={addBookmarkCurrent} title="Bookmark" className="opacity-70 hover:opacity-100"><Star size={14}/></button>
          </div>
        </form>

        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="Bookmarks" onClick={()=>{setPanel(p=>p==="bookmarks"?"none":"bookmarks"); loadBookmarks();}}><Star size={16}/></button>
        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="Downloads" onClick={()=>setPanel(p=>p==="downloads"?"none":"downloads")}><Download size={16}/></button>
        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="History" onClick={()=>{setPanel(p=>p==="history"?"none":"history"); loadHistory();}}><History size={16}/></button>
        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="Extensions"><Puzzle size={16}/></button>
        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="Private window" onClick={()=>addTab(settings.homepage,true)}><UserCircle2 size={16}/></button>
        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="Settings" onClick={()=>setPanel(p=>p==="settings"?"none":"settings")}><Settings size={16}/></button>
        <button className="p-1.5 rounded hover:bg-zinc-500/20" title="Theme" onClick={toggleTheme}>{dark?<Sun size={16}/>:<Moon size={16}/>}</button>
      </div>

      <div className="flex-1 flex min-h-0">
        {/* Content viewport host: native child webviews are positioned over this area */}
        <div ref={viewportRef} className="flex-1 relative bg-white">
          {!activeTab?.url || activeTab.url === "about:blank" ? (
            <div className={`absolute inset-0 flex flex-col items-center justify-center gap-4 ${dark?"bg-zinc-950 text-zinc-200":"bg-zinc-50 text-zinc-800"}`}>
              <div className="text-4xl font-bold tracking-tight">RUSTIUM</div>
              <div className="opacity-70 text-sm">A modern Rust-powered web browser (v0.1 foundation)</div>
              <form onSubmit={(e)=>{e.preventDefault(); navigateTo(address || settings.homepage);}} className="w-full max-w-xl px-6">
                <div className={`flex items-center gap-2 px-4 py-3 rounded-full border ${dark?"bg-zinc-900 border-zinc-700":"bg-white border-zinc-300"}`}>
                  <Search size={16} className="opacity-60"/><input value={address} onChange={e=>setAddress(e.target.value)} className="flex-1 bg-transparent outline-none" placeholder="Search or enter address"/>
                </div>
              </form>
              <div className="text-xs opacity-60 max-w-xl text-center px-6">Real web content renders in managed engine webviews (WebKitGTK/system webview). Some advanced browser features are staged per engine capabilities.</div>
            </div>
          ) : (
            <div className={`absolute inset-0 pointer-events-none ${dark?"bg-zinc-950":"bg-white"}`}></div>
          )}
        </div>

        {/* Side panel */}
        {panel !== "none" && (
          <div className={`w-80 border-l p-3 overflow-auto ${chrome}`}>
            <div className="flex items-center justify-between mb-3"><div className="font-semibold capitalize">{panel}</div><button onClick={()=>setPanel("none")}><X size={16}/></button></div>
            {panel==="history" && (
              <div className="space-y-2">
                <button className="text-xs px-2 py-1 rounded bg-red-500/20 text-red-300" onClick={async()=>{await invoke("clear_history"); loadHistory(); showToast("History cleared", "success");}}>Clear history</button>
                {history.length===0 && <div className="text-sm opacity-60">No history yet.</div>}
                {history.map(h => (
                  <div key={h.id} className="text-sm p-2 rounded hover:bg-zinc-500/10 cursor-pointer" onClick={()=>navigateTo(h.url)}>
                    <div className="font-medium truncate">{h.title || h.url}</div>
                    <div className="text-xs opacity-60 truncate">{h.url}</div>
                  </div>
                ))}
              </div>
            )}
            {panel==="bookmarks" && (
              <div className="space-y-2">
                <button className="text-xs px-2 py-1 rounded bg-blue-500/20 text-blue-300" onClick={addBookmarkCurrent}>Bookmark current page</button>
                {bookmarks.length===0 && <div className="text-sm opacity-60">No bookmarks yet.</div>}
                {bookmarks.map(b => (
                  <div key={b.id} className="text-sm p-2 rounded hover:bg-zinc-500/10 flex items-center gap-2">
                    <div className="flex-1 cursor-pointer" onClick={()=>navigateTo(b.url)}>
                      <div className="font-medium truncate">{b.title}</div>
                      <div className="text-xs opacity-60 truncate">{b.url}</div>
                    </div>
                    <button onClick={async()=>{await invoke("remove_bookmark",{id:b.id}); loadBookmarks(); showToast("Bookmark removed", "success");}} className="text-xs opacity-70">Remove</button>
                  </div>
                ))}
              </div>
            )}
            {panel==="downloads" && <div className="text-sm opacity-70">Downloads manager foundation: backend schema and UI panel scaffolded. File download pipeline integration is staged for engine/network wiring.</div>}
            {panel==="settings" && (
              <div className="space-y-3 text-sm">
                <label className="block">Homepage<input className="mt-1 w-full px-2 py-1 rounded bg-zinc-800 border border-zinc-700" value={settings.homepage} onChange={e=>setSettings({...settings, homepage:e.target.value})}/></label>
                <label className="block">Search engine prefix<input className="mt-1 w-full px-2 py-1 rounded bg-zinc-800 border border-zinc-700" value={settings.searchEngine} onChange={e=>setSettings({...settings, searchEngine:e.target.value})}/></label>
                <button className="px-3 py-1.5 rounded bg-emerald-600 text-white" onClick={async()=>{await invoke("save_settings",{settings});}}>Save settings</button>
                <div className="text-xs opacity-60">RUSTIUM v0.1.0 — Tauri/Wry system webview engine (real rendering). See docs/ARCHITECTURE.md for capabilities and limitations.</div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Toast notifications */}
      {toast && (
        <div className={`fixed bottom-4 right-4 px-4 py-3 rounded-lg shadow-lg z-50 ${
          toast.type === 'success' ? 'bg-emerald-600 text-white' : 'bg-red-600 text-white'
        }`}>
          {toast.message}
        </div>
      )}
    </div>
  );
}
