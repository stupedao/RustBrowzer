import { useEffect, useState, type FormEvent } from "react";
import { BookOpen, Code, Globe, Lock, Newspaper, Play, Search, Sparkles, Terminal } from "lucide-react";

type HomeProps = {
  dark: boolean;
  engineName: string;
  isPrivate: boolean;
  onNavigate: (input: string) => void;
};

const QUICK_LINKS = [
  { label: "Wikipedia", url: "https://www.wikipedia.org", Icon: BookOpen },
  { label: "GitHub", url: "https://github.com", Icon: Terminal },
  { label: "MDN", url: "https://developer.mozilla.org", Icon: Code },
  { label: "Hacker News", url: "https://news.ycombinator.com", Icon: Newspaper },
  { label: "Rust", url: "https://www.rust-lang.org", Icon: Globe },
  { label: "YouTube", url: "https://www.youtube.com", Icon: Play },
];

function greetingFor(hour: number): string {
  if (hour < 5) return "Still up";
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

/**
 * The start page.
 *
 * Performance notes:
 * - The clock interval is aligned to the next minute boundary (one tick per
 *   minute, not per second/20s).
 * - No persistent animations and no large blur radii (blur is one of the most
 *   expensive CSS effects to composite); the accent is a simple static
 *   gradient.
 */
export default function Home({ dark, engineName, isPrivate, onNavigate }: HomeProps) {
  const [query, setQuery] = useState("");
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    // Align the timer to the next minute so the clock only re-renders when
    // the shown value actually changes.
    const msInMin = 60_000;
    let interval = 0;
    const timeout = window.setTimeout(() => {
      setNow(new Date());
      interval = window.setInterval(() => setNow(new Date()), msInMin);
    }, msInMin - (Date.now() % msInMin) + 20);
    return () => {
      window.clearTimeout(timeout);
      if (interval) window.clearInterval(interval);
    };
  }, []);

  const time = now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const date = now.toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" });

  function submit(e: FormEvent) {
    e.preventDefault();
    const q = query.trim();
    if (q) onNavigate(q);
  }

  const shell = dark ? "bg-zinc-950 text-zinc-100" : "bg-zinc-50 text-zinc-900";
  const sub = dark ? "text-zinc-400" : "text-zinc-500";
  const tile = dark
    ? "border-zinc-800 bg-zinc-900 hover:border-zinc-700 hover:bg-zinc-800/70"
    : "border-zinc-200 bg-white hover:border-zinc-300";
  const searchBox = dark
    ? "border-zinc-700 bg-zinc-900 focus-within:border-sky-500"
    : "border-zinc-300 bg-white focus-within:border-sky-500";

  return (
    <div className={`absolute inset-0 overflow-auto ${shell}`}>
      <div className="mx-auto flex min-h-full w-full max-w-2xl flex-col px-6">
        {/* clock header */}
        <header className="flex items-start justify-between py-6">
          <div className={`flex items-center gap-2 text-xs ${sub}`}>
            <Lock size={11} className={isPrivate ? "text-violet-400" : "text-emerald-500"} />
            {isPrivate ? "Private tab — nothing is saved" : "Private by design"}
          </div>
          <div className="text-right">
            <div className="text-2xl font-light tabular-nums">{time}</div>
            <div className={`mt-0.5 text-[11px] uppercase tracking-widest ${sub}`}>{date}</div>
          </div>
        </header>

        {/* search */}
        <main className="flex flex-1 flex-col items-center justify-center pb-10">
          <h1 className="text-3xl font-light tracking-tight sm:text-4xl">
            {greetingFor(now.getHours())}
            <span className="text-sky-500">.</span>
          </h1>
          <p className={`mt-2 text-sm ${sub}`}>Where do you want to go?</p>

          <form onSubmit={submit} className="mt-8 w-full" role="search">
            <div className={`flex h-12 items-center gap-3 rounded-2xl border px-4 transition-colors ${searchBox}`}>
              <Search size={17} className="shrink-0 opacity-40" />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={`Search ${engineName} or type a URL`}
                spellCheck={false}
                autoComplete="off"
                aria-label="Search or enter address"
                className="min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:opacity-40"
              />
              <kbd className={`hidden rounded-md border px-1.5 py-0.5 text-[10px] sm:block
                ${dark ? "border-zinc-700 text-zinc-500" : "border-zinc-300 text-zinc-400"}`}>
                Enter
              </kbd>
            </div>
          </form>

          {/* quick links */}
          <div className="mt-10 w-full">
            <div className={`mb-3 flex items-center justify-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.2em] ${sub}`}>
              <Sparkles size={11} />
              Quick links
            </div>
            <div className="grid grid-cols-3 gap-2 sm:grid-cols-6">
              {QUICK_LINKS.map(({ label, url, Icon }) => (
                <button
                  key={label}
                  type="button"
                  title={url}
                  onClick={() => onNavigate(url)}
                  className={`flex flex-col items-center gap-2 rounded-xl border px-2 py-3 transition-colors ${tile}`}
                >
                  <Icon size={16} className="opacity-70" />
                  <span className="w-full truncate text-center text-[11px] font-medium">{label}</span>
                </button>
              ))}
            </div>
          </div>
        </main>

        {/* footer */}
        <footer className={`flex flex-wrap items-center justify-between gap-2 border-t py-3 text-[10px] uppercase tracking-[0.15em] ${dark ? "border-zinc-800" : "border-zinc-200"} ${sub}`}>
          <span>
            <kbd className="rounded border px-1 py-0.5 tracking-normal">Ctrl T</kbd> new tab
            <span className="mx-2">·</span>
            <kbd className="rounded border px-1 py-0.5 tracking-normal">Ctrl L</kbd> address
            <span className="mx-2">·</span>
            <kbd className="rounded border px-1 py-0.5 tracking-normal">Ctrl W</kbd> close
          </span>
          <span>{isPrivate ? "Private session" : "Low-memory mode"}</span>
        </footer>
      </div>
    </div>
  );
}
