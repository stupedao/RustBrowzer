import { useEffect, useState, type FormEvent } from "react";
import {
  ArrowUpRight, BookOpen, Code, Globe, Lock, Newspaper, Play, Search,
  Sparkles, Terminal,
} from "lucide-react";

type HomeProps = {
  dark: boolean;
  searchEngine: string;
  isPrivate: boolean;
  onNavigate: (input: string) => void;
};

const QUICK_LINKS = [
  { label: "GitHub", host: "github.com", url: "https://github.com", Icon: Terminal, tint: "from-zinc-500 to-zinc-700" },
  { label: "Wikipedia", host: "wikipedia.org", url: "https://www.wikipedia.org", Icon: BookOpen, tint: "from-slate-400 to-slate-600" },
  { label: "Rust", host: "rust-lang.org", url: "https://www.rust-lang.org", Icon: Code, tint: "from-orange-500 to-rose-600" },
  { label: "Hacker News", host: "news.ycombinator.com", url: "https://news.ycombinator.com", Icon: Newspaper, tint: "from-amber-500 to-orange-600" },
  { label: "MDN", host: "developer.mozilla.org", url: "https://developer.mozilla.org", Icon: Globe, tint: "from-sky-500 to-indigo-600" },
  { label: "YouTube", host: "youtube.com", url: "https://www.youtube.com", Icon: Play, tint: "from-rose-500 to-red-600" },
];

function engineLabel(prefix: string) {
  try {
    const host = new URL(prefix).hostname.replace(/^www\./, "");
    if (host.startsWith("google.")) return "Google";
    if (host.startsWith("duckduckgo.")) return "DuckDuckGo";
    if (host.startsWith("bing.")) return "Bing";
    return host;
  } catch {
    return "the web";
  }
}

function greetingFor(hour: number) {
  if (hour < 5) return "Still up";
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

export default function Home({ dark, searchEngine, isPrivate, onNavigate }: HomeProps) {
  const [query, setQuery] = useState("");
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 20_000);
    return () => window.clearInterval(timer);
  }, []);

  const time = now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const date = now.toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" });
  const engine = engineLabel(searchEngine);

  const palette = dark
    ? {
        shell: "bg-[#07070a] text-zinc-100",
        eyebrow: "text-zinc-500",
        heading: "text-zinc-50",
        sub: "text-zinc-400",
        chip: "border-white/[0.08] bg-white/[0.03] text-zinc-400",
        search: "border-white/10 bg-white/[0.04] hover:border-white/20 focus-within:border-amber-400/50 focus-within:bg-white/[0.06] focus-within:shadow-amber-500/10",
        input: "placeholder:text-zinc-500",
        kbd: "border-white/10 bg-white/[0.04] text-zinc-400",
        tile: "border-white/[0.06] bg-white/[0.025] hover:border-white/[0.14] hover:bg-white/[0.055]",
        tileText: "text-zinc-300",
        grid: "linear-gradient(to right, rgba(255,255,255,0.55) 1px, transparent 1px), linear-gradient(to bottom, rgba(255,255,255,0.55) 1px, transparent 1px)",
        glow: "opacity-[0.34]",
        footer: "text-zinc-500",
        rule: "border-white/[0.06]",
      }
    : {
        shell: "bg-[#f6f5f2] text-zinc-900",
        eyebrow: "text-zinc-500",
        heading: "text-zinc-900",
        sub: "text-zinc-500",
        chip: "border-black/[0.08] bg-white/70 text-zinc-500",
        search: "border-black/10 bg-white hover:border-black/20 focus-within:border-amber-500/60 focus-within:shadow-amber-500/10",
        input: "placeholder:text-zinc-400",
        kbd: "border-black/10 bg-black/[0.03] text-zinc-500",
        tile: "border-black/[0.07] bg-white hover:border-black/15 hover:bg-white",
        tileText: "text-zinc-700",
        grid: "linear-gradient(to right, rgba(0,0,0,0.5) 1px, transparent 1px), linear-gradient(to bottom, rgba(0,0,0,0.5) 1px, transparent 1px)",
        glow: "opacity-[0.22]",
        footer: "text-zinc-500",
        rule: "border-black/[0.07]",
      };

  function submit(e: FormEvent) {
    e.preventDefault();
    onNavigate(query.trim() || "rustium://home");
  }

  return (
    <div className={`absolute inset-0 overflow-auto ${palette.shell}`}>
      {/* Ambient background */}
      <div aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden">
        <div
          className={`absolute -top-48 left-1/2 h-[520px] w-[900px] -translate-x-1/2 rounded-full blur-[130px] ${palette.glow}`}
          style={{
            background:
              "radial-gradient(circle at 32% 42%, #f97316 0%, transparent 55%), radial-gradient(circle at 68% 38%, #34d399 0%, transparent 52%)",
          }}
        />
        <div
          className="absolute inset-0 opacity-[0.045]"
          style={{
            backgroundImage: palette.grid,
            backgroundSize: "56px 56px",
            maskImage: "radial-gradient(ellipse at 50% 38%, black 0%, transparent 72%)",
            WebkitMaskImage: "radial-gradient(ellipse at 50% 38%, black 0%, transparent 72%)",
          }}
        />
        <div
          className="absolute inset-x-0 bottom-0 h-40"
          style={{ background: dark ? "linear-gradient(to top, #07070a, transparent)" : "linear-gradient(to top, #f6f5f2, transparent)" }}
        />
      </div>

      <div className="relative flex min-h-full flex-col">
        {/* Brand + clock */}
        <header className="flex items-start justify-between px-7 pb-2 pt-6">
          <div className="flex items-center gap-3">
            <div className="relative h-9 w-9">
              <div className="absolute inset-0 rounded-[12px] bg-[conic-gradient(from_210deg,#fb923c,#f59e0b,#34d399,#fb923c)] opacity-70 blur-[7px]" />
              <div className="relative h-9 w-9 rounded-[12px] bg-[conic-gradient(from_210deg,#fb923c,#f59e0b,#34d399,#fb923c)] p-[1.5px]">
                <div className="flex h-full w-full items-center justify-center rounded-[10.5px] bg-[#0b0b0e]">
                  <span className="text-[13px] font-bold leading-none tracking-tight text-white">R</span>
                </div>
              </div>
            </div>
            <div className="leading-tight">
              <div className={`text-[11px] font-semibold tracking-[0.34em] ${palette.heading}`}>RUSTIUM</div>
              <div className={`mt-0.5 text-[9px] tracking-[0.22em] ${palette.eyebrow}`}>HOME</div>
            </div>
          </div>

          <div className="text-right">
            <div className={`text-[26px] font-light leading-none tracking-tight tabular-nums ${palette.heading}`}>{time}</div>
            <div className={`mt-1.5 text-[10px] uppercase tracking-[0.18em] ${palette.eyebrow}`}>{date}</div>
          </div>
        </header>

        {/* Center stage */}
        <main className="flex flex-1 flex-col items-center justify-center px-6 py-4">
          <div className="home-rise flex w-full max-w-2xl flex-col items-center text-center">
            <div className={`mb-3 inline-flex items-center gap-2 rounded-full border px-3 py-1 text-[10px] uppercase tracking-[0.2em] ${palette.chip}`}>
              <span className="relative flex h-1.5 w-1.5">
                <span className={`absolute inline-flex h-full w-full animate-ping rounded-full ${isPrivate ? "bg-violet-400" : "bg-emerald-400"} opacity-70`} />
                <span className={`relative inline-flex h-1.5 w-1.5 rounded-full ${isPrivate ? "bg-violet-400" : "bg-emerald-400"}`} />
              </span>
              {isPrivate ? "Private session" : "System ready"}
            </div>

            <h1 className={`text-[34px] font-light leading-tight tracking-[-0.025em] sm:text-[42px] ${palette.heading}`}>
              {greetingFor(now.getHours())}
              <span className="text-amber-400">.</span>
            </h1>
            <p className={`mt-1.5 text-sm ${palette.sub}`}>Where do you want to go?</p>

            <form onSubmit={submit} className="mt-7 w-full">
              <div className={`flex items-center gap-3 rounded-2xl border px-4 py-3.5 shadow-2xl shadow-black/20 outline-none transition-all duration-200 ${palette.search}`}>
                <Search size={18} className="shrink-0 text-amber-400/90" />
                <input
                  autoFocus
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder={`Search ${engine} or type a URL`}
                  spellCheck={false}
                  autoComplete="off"
                  aria-label={`Search ${engine} or type a URL`}
                  className={`min-w-0 flex-1 bg-transparent text-[15px] font-light outline-none ${palette.input}`}
                />
                <kbd className={`hidden shrink-0 items-center gap-1 rounded-md border px-2 py-1 text-[10px] font-medium tracking-wide sm:flex ${palette.kbd}`}>
                  Enter <span className="opacity-50">↵</span>
                </kbd>
              </div>
              <div className={`mt-2.5 flex items-center justify-center gap-2 text-[10px] uppercase tracking-[0.16em] ${palette.footer}`}>
                <Lock size={10} className="text-emerald-500/80" />
                <span>{isPrivate ? "Nothing is being recorded" : "Encrypted connection"} · WebKitGTK</span>
              </div>
            </form>

            <div className="mt-8 w-full">
              <div className={`mb-3 flex items-center justify-center gap-2 text-[9px] uppercase tracking-[0.26em] ${palette.eyebrow}`}>
                <Sparkles size={11} className="text-amber-400/80" />
                Pinned destinations
              </div>
              <div className="grid grid-cols-3 gap-2.5 sm:grid-cols-6">
                {QUICK_LINKS.map(({ label, host, url, Icon, tint }) => (
                  <button
                    key={label}
                    type="button"
                    onClick={() => onNavigate(url)}
                    title={url}
                    className={`group flex flex-col items-center gap-1.5 rounded-2xl border px-2 py-3 transition-all duration-200 hover:-translate-y-0.5 ${palette.tile}`}
                  >
                    <span className={`flex h-9 w-9 items-center justify-center rounded-xl bg-gradient-to-br ${tint} text-white shadow-lg shadow-black/20 transition-transform duration-200 group-hover:scale-105`}>
                      <Icon size={16} strokeWidth={2} />
                    </span>
                    <span className={`w-full truncate text-[11px] font-medium tracking-wide ${palette.tileText}`}>{label}</span>
                    <span className="flex w-full items-center justify-center gap-0.5 truncate text-[9px] opacity-35">
                      <span className="truncate">{host}</span>
                      <ArrowUpRight size={8} className="shrink-0 opacity-0 transition-opacity group-hover:opacity-100" />
                    </span>
                  </button>
                ))}
              </div>
            </div>
          </div>
        </main>

        {/* Shortcuts */}
        <footer className={`flex flex-wrap items-center justify-between gap-x-4 gap-y-2 border-t px-7 py-3 text-[9px] uppercase tracking-[0.18em] ${palette.rule} ${palette.footer}`}>
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5">
            <span className="flex items-center gap-1.5"><kbd className={`rounded border px-1.5 py-0.5 text-[9px] tracking-normal ${palette.kbd}`}>Ctrl T</kbd> New tab</span>
            <span className="flex items-center gap-1.5"><kbd className={`rounded border px-1.5 py-0.5 text-[9px] tracking-normal ${palette.kbd}`}>Ctrl L</kbd> Address</span>
            <span className="flex items-center gap-1.5"><kbd className={`rounded border px-1.5 py-0.5 text-[9px] tracking-normal ${palette.kbd}`}>Ctrl B</kbd> Bookmarks</span>
          </div>
          <div className="flex items-center gap-1.5">
            <Terminal size={10} />
            <span>Rust-powered · v0.1.0</span>
          </div>
        </footer>
      </div>
    </div>
  );
}
