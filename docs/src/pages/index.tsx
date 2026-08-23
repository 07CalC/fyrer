import React from 'react';
import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';

// ──────────────────────────────────────────────────────────────────────────────
// helpers
// ──────────────────────────────────────────────────────────────────────────────
function useCopy() {
  const [copied, setCopied] = React.useState(false);
  const copy = React.useCallback(async (t: string) => {
    try {
      await navigator.clipboard.writeText(t);
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    } catch {}
  }, []);
  return {copied, copy};
}

function useReveal() {
  const ref = React.useRef<HTMLDivElement>(null);
  const [visible, setVisible] = React.useState(false);
  React.useEffect(() => {
    if (!ref.current) return;
    const obs = new IntersectionObserver(
      ([e]) => {
        if (e.isIntersecting) {
          setVisible(true);
          obs.disconnect();
        }
      },
      {threshold: 0.12, rootMargin: '0px 0px -40px 0px'},
    );
    obs.observe(ref.current);
    return () => obs.disconnect();
  }, []);
  return {ref, visible};
}

function useScrolled() {
  React.useEffect(() => {
    const nav = document.querySelector('.navbar') as HTMLElement | null;
    if (!nav) return;
    const onScroll = () => {
      if (window.scrollY > 8) nav.classList.add('scrolled');
      else nav.classList.remove('scrolled');
    };
    onScroll();
    window.addEventListener('scroll', onScroll, {passive: true});
    return () => window.removeEventListener('scroll', onScroll);
  }, []);
}

// ──────────────────────────────────────────────────────────────────────────────
// tiny UI
// ──────────────────────────────────────────────────────────────────────────────
function Eyebrow() {
  return (
    <div className="inline-flex items-center gap-2 rounded-full border border-[#1c1f22] bg-[#0f1113] px-3 py-1.5">
      <span className="relative flex h-2 w-2">
        <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[#ff4d00] opacity-30" />
        <span className="relative inline-flex h-2 w-2 rounded-full bg-[#ff4d00]" />
      </span>
      <span className="font-mono text-[11px] font-medium tracking-wide text-[#8a8f98]">Open source</span>
      <span className="h-3 w-px bg-[#1c1f22]" />
      <span className="font-mono text-[11px] font-medium tracking-wide text-[#8a8f98]">Built for developers</span>
    </div>
  );
}

function SectionLabel({k, label}: {k: string; label: string}) {
  return (
    <div className="flex items-center gap-3">
      <span className="font-mono text-[11px] font-medium tracking-[0.14em] text-[#ff4d00]">{k}</span>
      <span className="h-px w-8 bg-[#1c1f22]" />
      <span className="font-mono text-[11px] font-medium tracking-[0.14em] text-[#5a5f68] uppercase">{label}</span>
    </div>
  );
}

function CopyBtn({text, small}: {text: string; small?: boolean}) {
  const {copied, copy} = useCopy();
  return (
    <button
      onClick={() => copy(text)}
      aria-label="Copy"
      className={`inline-flex items-center justify-center rounded-md border border-[#252a2e] bg-[#1c1f22] font-mono font-medium text-[#8a8f98] transition hover:border-[#2e3539] hover:bg-[#242a2e] hover:text-[#eceef0] ${small ? 'h-7 px-2.5 text-[11px]' : 'h-7 px-3 text-xs'}`}
    >
      {copied ? 'Copied' : 'Copy'}
    </button>
  );
}

// ──────────────────────────────────────────────────────────────────────────────
// terminal — simplified but still animated
// ──────────────────────────────────────────────────────────────────────────────
function HeroTerminal() {
  const [typed, setTyped] = React.useState(0);
  const [showOutput, setShowOutput] = React.useState(false);
  const cmd = 'fyrer run build';
  React.useEffect(() => {
    let i = 0;
    const id = setInterval(() => {
      i += 1;
      setTyped(i);
      if (i >= cmd.length) {
        clearInterval(id);
        setTimeout(() => setShowOutput(true), 360);
      }
    }, 45);
    return () => clearInterval(id);
  }, []);
  return (
    <div className="overflow-hidden rounded-[14px] border border-[#1c1f22] bg-[#0f1113] shadow-[0_12px_40px_rgba(0,0,0,0.5)]">
      <div className="flex items-center gap-1.5 border-b border-[#1c1f22] bg-[#0f1113] px-3 py-2.5">
        <span className="h-3 w-3 rounded-full border border-[#252a2e] bg-[#141718]" />
        <span className="h-3 w-3 rounded-full border border-[#252a2e] bg-[#141718]" />
        <span className="h-3 w-3 rounded-full border border-[#252a2e] bg-[#141718]" />
        <span className="ml-3 font-mono text-[11px] tracking-wide text-[#5a5f68]">fyrer run build — acme-corp</span>
        <span className="ml-auto hidden items-center gap-2 sm:flex">
          <span className="h-1.5 w-1.5 rounded-full bg-[#10b981]" />
          <span className="font-mono text-[11px] text-[#5a5f68]">EXIT 0</span>
          <span className="rounded bg-[#141718] px-2 py-0.5 font-mono text-[11px] font-medium text-[#8a8f98]">2.10s</span>
        </span>
      </div>
      <div className="relative bg-[#08090a] p-4 font-mono text-[12.5px] leading-[1.6] sm:p-5">
        <div className="flex items-center gap-2">
          <span className="text-[#5a5f68]">$</span>
          <span className="text-[#eceef0]">{cmd.slice(0, typed)}</span>
          <span className={`inline-block h-[14px] w-[7px] bg-[#eceef0] ${showOutput ? 'opacity-0' : 'animate-pulse'}`} style={{marginLeft: typed < cmd.length ? 1 : -2}} />
          {!showOutput && <span className="ml-2 hidden text-[#5a5f68] sm:inline">— streaming DAG</span>}
        </div>

        <div className={`mt-4 space-y-2.5 transition-all duration-500 ${showOutput ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-1'}`}>
          <div className="flex items-center gap-2 text-[12px]">
            <span className="rounded bg-[#1c1f22] px-1.5 py-0.5 text-[11px] font-medium text-[#8a8f98]">ui:build</span>
            <span className="truncate text-[#a8adb5]">bun build src --outdir dist</span>
            <span className="ml-auto hidden items-center gap-1 text-[#10b981] sm:inline-flex">✓ 310ms</span>
          </div>
          <div className="flex items-center gap-2 text-[12px]">
            <span className="rounded bg-[#1c1f22] px-1.5 py-0.5 text-[11px] font-medium text-[#8a8f98]">api:build</span>
            <span className="truncate text-[#a8adb5]">cargo build --release</span>
            <span className="ml-auto hidden items-center gap-1 text-[#10b981] sm:inline-flex">✓ 2.10s</span>
          </div>
          <div className="relative ml-6 flex items-center gap-2 border-l border-dashed border-[#ff4d00]/25 pl-3">
            <span className="absolute -left-[3px] top-1/2 h-1.5 w-1.5 -translate-y-1/2 rounded-full bg-[#ff4d00]" />
            <span className="rounded bg-[rgba(255,77,0,0.1)] px-1.5 py-0.5 text-[11px] font-medium text-[#ff6b2b]">web:build</span>
            <span className="truncate text-[#a8adb5]">bun build src --outdir dist</span>
            <span className="ml-auto hidden rounded bg-[rgba(255,77,0,0.1)] px-2 py-0.5 text-[10px] font-medium text-[#ff6b2b] sm:inline">310ms → start</span>
          </div>
          <div className="ml-6 border-l border-[#1c1f22] pl-3 font-mono text-[11px] text-[#5a5f68]">only waited on ui:build — not api:build</div>

          <div className="pt-3">
            <div className="flex items-center gap-2 rounded-[10px] border border-[#1c1f22] bg-[#0f1113] px-3 py-2.5">
              <span className="h-2 w-2 rounded-full bg-[#10b981]" />
              <span className="font-mono text-[12px] font-medium text-[#eceef0]">2.10s</span>
              <span className="font-mono text-[11px] text-[#10b981]">✓3</span>
              <span className="font-mono text-[11px] text-[#5a5f68]">⚡0</span>
              <span className="ml-auto hidden font-mono text-[11px] text-[#5a5f68] sm:inline">streaming • DAG</span>
            </div>
          </div>

          <div className="flex flex-wrap gap-2 pt-1">
            <span className="rounded bg-[rgba(255,77,0,0.08)] px-2 py-1 text-[11px] font-medium text-[#ff6b2b]">⚡ 0.04s on second run</span>
            <span className="rounded border border-[#1c1f22] bg-[#0f1113] px-2 py-1 text-[11px] text-[#5a5f68]">ALL CACHED</span>
          </div>
        </div>

        <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(to_right,rgba(255,255,255,0.01)_1px,transparent_1px),linear-gradient(to_bottom,rgba(255,255,255,0.01)_1px,transparent_1px)] bg-[size:24px_24px] opacity-30" />
      </div>
      <div className="flex items-center justify-between border-t border-[#1c1f22] bg-[#0f1113] px-3 py-2 font-mono text-[11px] text-[#5a5f68]">
        <span>.fyrer/cache • blake3 • tar.zst</span>
        <span className="hidden sm:inline">no daemon • sh -c</span>
      </div>
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────────
// product viz — simplified
// ──────────────────────────────────────────────────────────────────────────────
function ProductViz() {
  const {ref, visible} = useReveal();
  return (
    <div
      ref={ref}
      className={`overflow-hidden rounded-[14px] border border-[#1c1f22] bg-[#0f1113] shadow-[0_12px_40px_rgba(0,0,0,0.45)] transition-all duration-700 ${visible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-3'}`}
    >
      <div className="flex items-center gap-2 border-b border-[#1c1f22] bg-[#08090a] px-3 py-2.5">
        <div className="flex items-center gap-1.5">
          <span className="h-2.5 w-2.5 rounded-full border border-[#252a2e] bg-[#141718]" />
          <span className="h-2.5 w-2.5 rounded-full border border-[#252a2e] bg-[#141718]" />
          <span className="h-2.5 w-2.5 rounded-full border border-[#252a2e] bg-[#141718]" />
        </div>
        <span className="ml-2 font-mono text-[11px] tracking-wide text-[#5a5f68]">fyrer plan — acme-corp</span>
        <span className="ml-auto hidden items-center gap-2 sm:flex">
          <span className="h-1.5 w-1.5 rounded-full bg-[#10b981] animate-pulse" />
          <span className="font-mono text-[11px] text-[#5a5f68]">DAG resolved</span>
        </span>
      </div>

      <div className="grid bg-[#08090a] lg:grid-cols-[300px_1fr]">
        <div className="border-b border-[#1c1f22] bg-[#0f1113] p-4 lg:border-b-0 lg:border-r">
          <div className="flex items-center justify-between">
            <span className="font-mono text-[11px] font-medium tracking-[0.08em] text-[#5a5f68] uppercase">Graph</span>
            <span className="rounded bg-[#141718] px-1.5 py-0.5 font-mono text-[11px] text-[#5a5f68]">fyrer.yml</span>
          </div>

          <div className="mt-4 rounded-[10px] border border-[#1c1f22] bg-[#08090a] p-3 font-mono text-[11.5px] leading-6">
            <div className="flex items-center gap-2">
              <span className="h-1.5 w-1.5 rounded-full bg-[#ff4d00]" />
              <span className="text-[#8a8f98]">acme-corp</span>
              <span className="ml-auto text-[11px] text-[#5a5f68]">4 pkgs</span>
            </div>
            <div className="mt-3 space-y-1.5">
              <div className="flex items-center gap-2">
                <span className="text-[#252a2e]">├─</span>
                <span className="rounded bg-[#1c1f22] px-1.5 py-0 text-[11px] text-[#c8ccd2]">ui:build</span>
                <span className="ml-auto text-[11px] text-[#10b981]">310ms</span>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-[#252a2e]">├─</span>
                <span className="rounded bg-[#1c1f22] px-1.5 py-0 text-[11px] text-[#c8ccd2]">api:build</span>
                <span className="ml-auto text-[11px] text-[#10b981]">2.10s</span>
              </div>
              <div className="flex items-center gap-2 pl-4 text-[#8a8f98]">
                <span className="text-[#252a2e]">└─</span>
                <span className="rounded bg-[rgba(255,77,0,0.1)] px-1.5 py-0 text-[11px] font-medium text-[#ff6b2b]">web:build</span>
                <span className="text-[#5a5f68]">→ ui</span>
              </div>
            </div>
          </div>

          <div className="mt-3 flex items-center gap-2 rounded-[10px] border border-[rgba(255,77,0,0.12)] bg-[rgba(255,77,0,0.06)] px-3 py-2.5">
            <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-[#ff4d00]" />
            <span className="font-mono text-[11px] leading-4 text-[#ff6b2b]">web started at 310ms — 1.79s saved</span>
          </div>
        </div>

        <div className="min-w-0 bg-[#08090a] p-5">
          <div className="flex items-center justify-between">
            <span className="font-mono text-[11px] font-medium tracking-[0.08em] text-[#5a5f68] uppercase">Timeline</span>
            <span className="font-mono text-[11px] text-[#5a5f68]">wall-clock 2.10s</span>
          </div>

          <div className="mt-5 space-y-3">
            {[
              {name: 'ui:build', w: '15%', color: 'bg-[#ff4d00]', label: '310ms', start: '0%'},
              {name: 'web:build', w: '20%', color: 'bg-[#eceef0]', label: '420ms', start: '15%'},
              {name: 'api:build', w: '88%', color: 'bg-[#252a2e]', label: '2.10s', start: '0%'},
            ].map((r) => (
              <div key={r.name} className="flex items-center gap-3">
                <span className="w-[84px] shrink-0 text-right font-mono text-[11px] font-medium text-[#8a8f98]">{r.name}</span>
                <div className="relative h-6 flex-1 overflow-hidden rounded-full bg-[#0f1113] p-1">
                  <div className={`absolute top-1 h-4 rounded-full ${r.color}`} style={{left: r.start, width: r.w}} />
                  <span className="absolute top-1/2 -translate-y-1/2 font-mono text-[10px] font-medium text-white mix-blend-difference" style={{left: `calc(${r.start} + 8px)`}}>
                    {r.label}
                  </span>
                </div>
              </div>
            ))}
          </div>

          <div className="mt-6 rounded-[10px] border border-[#1c1f22] bg-[#0f1113] p-3 font-mono text-[11px] leading-5">
            <div className="flex gap-2 text-[#5a5f68] pb-2 border-b border-[#1c1f22]">
              <span>[ui:build]</span>
              <span className="truncate text-[#a8adb5]">bundled 42 modules</span>
              <span className="ml-auto text-[#10b981]">✓</span>
            </div>
            <div className="flex gap-2 pt-2 text-[#ff6b2b]">
              <span>[web:build]</span>
              <span className="truncate text-[#a8adb5]">starting — ui satisfied</span>
              <span className="ml-auto rounded bg-[rgba(255,77,0,0.12)] px-1 text-[#ff6b2b]">↻</span>
            </div>
          </div>
        </div>
      </div>

      <div className="flex items-center justify-between border-t border-[#1c1f22] bg-[#0f1113] px-3 py-2 font-mono text-[11px] text-[#5a5f68]">
        <span>streaming • concurrency 8</span>
        <span className="hidden sm:inline">DAG • cache miss 2.10s → hit 0.04s</span>
      </div>
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────────
// features — reduced to 6 for glanceability
// ──────────────────────────────────────────────────────────────────────────────
const FEATURES = [
  {
    icon: '◈',
    title: 'Streaming DAG',
    desc: 'No barriers. Tasks start the moment deps succeed — up to concurrency.',
    meta: '310ms start',
    code: 'depends_on: [ui:build]',
  },
  {
    icon: '⬢',
    title: 'blake3 cache',
    desc: 'Hash of id + cmd + inputs → tar.zst. Second run restores instantly.',
    meta: '0.04s',
    code: 'cache: true',
  },
  {
    icon: '›_',
    title: 'Any shell command',
    desc: 'If it runs via sh -c, fyrer runs it. Any toolchain, any runtime — no plugins.',
    meta: 'sh -c',
    code: 'cmd: ./scripts/build.sh',
  },
  {
    icon: '◐',
    title: 'Watch + persistent',
    desc: 'Poll + debounce 300ms. Dev servers stay alive until you quit.',
    meta: '300ms',
    code: 'watch: true',
  },
  {
    icon: '▣',
    title: 'TUI & plain',
    desc: 'Interactive panes with r/K/q — or -n for CI logs.',
    meta: 'r / K / q',
    code: 'fyrer run -n',
  },
  {
    icon: '⬣',
    title: 'Single binary',
    desc: 'curl | sh or cargo install. 6 targets. No daemon.',
    meta: '6 targets',
    code: 'cargo install fyrer',
  },
];

function FeatureCard({f}: {f: (typeof FEATURES)[number]}) {
  const {ref, visible} = useReveal();
  return (
    <div
      ref={ref}
      className={`group flex flex-col rounded-[12px] border border-[#1c1f22] bg-[#0f1113] p-5 transition-all duration-500 hover:border-[#252a2e] hover:bg-[#141718] ${visible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-2'}`}
    >
      <div className="flex items-center justify-between">
        <span className="flex h-8 w-8 items-center justify-center rounded-[8px] border border-[#1c1f22] bg-[#08090a] font-mono text-[13px] text-[#8a8f98] group-hover:text-[#eceef0]">{f.icon}</span>
        <span className="rounded-full border border-[#1c1f22] bg-[#08090a] px-2.5 py-1 font-mono text-[11px] text-[#5a5f68]">{f.meta}</span>
      </div>
      <h3 className="mt-4 font-sans text-[14px] font-semibold tracking-tight text-[#eceef0]">{f.title}</h3>
      <p className="mt-2 flex-1 font-sans text-[13px] leading-6 text-[#8a8f98]">{f.desc}</p>
      <div className="mt-4 rounded-[8px] border border-[#1c1f22] bg-[#08090a] px-2.5 py-2 font-mono text-[11px] text-[#5a5f68]">{f.code}</div>
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────────
// code panel
// ──────────────────────────────────────────────────────────────────────────────
function CodePanel({title, lang, code, meta}: {title: string; lang: string; code: string; meta?: string}) {
  const {copied, copy} = useCopy();
  return (
    <div className="overflow-hidden rounded-[12px] border border-[#1c1f22] bg-[#0f1113]">
      <div className="flex items-center gap-2 border-b border-[#1c1f22] bg-[#0f1113] px-3 py-2.5">
        <span className="font-mono text-[11px] font-medium tracking-wide text-[#5a5f68] uppercase">{title}</span>
        {meta && <span className="hidden font-mono text-[11px] text-[#5a5f68] sm:inline">— {meta}</span>}
        <span className="ml-auto rounded bg-[#141718] px-2 py-1 font-mono text-[10px] font-medium tracking-wide text-[#5a5f68]">{lang}</span>
        <button onClick={() => copy(code)} className="rounded-md border border-[#1c1f22] bg-[#141718] px-2.5 py-1 font-mono text-[11px] font-medium text-[#8a8f98] hover:text-[#eceef0] hover:border-[#252a2e]">
          {copied ? 'Copied' : 'Copy'}
        </button>
      </div>
      <pre className="overflow-x-auto p-4 font-mono text-[12.5px] leading-6 text-[#a8adb5] sm:p-5">
        <code className="whitespace-pre">{code}</code>
      </pre>
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────────
// main page — de-cluttered for one-glance reading
// ──────────────────────────────────────────────────────────────────────────────
export default function Home() {
  useScrolled();
  const INSTALL = 'curl -fsSL https://raw.githubusercontent.com/07calc/fyrer/main/install.sh | sh';
  const INSTALL_SHORT = 'curl -fsSL fyrer.sh | sh';

  return (
    <Layout title="Run everything. Wait for nothing." description="Streaming DAG for monorepos. One fyrer.yml for any language, any stack — blake3 cache, 300ms watch, single binary. No plugins.">
      <div className="min-w-0 overflow-x-hidden bg-[#08090a] text-[#eceef0]">
        {/* 1 — HERO — CLEAN */}
        <section className="relative isolate overflow-hidden border-b border-[#141718] bg-[#08090a]">
          <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(to_right,rgba(255,255,255,0.015)_1px,transparent_1px),linear-gradient(to_bottom,rgba(255,255,255,0.015)_1px,transparent_1px)] bg-[size:32px_32px]" />
          <div className="pointer-events-none absolute -top-32 left-1/2 h-[600px] w-[900px] -translate-x-1/2 rounded-full bg-[radial-gradient(ellipse_at_center,rgba(255,77,0,0.06),transparent_70%)]" />

          <div className="relative mx-auto w-full max-w-[1160px] px-4 py-10 sm:px-6 sm:py-14 lg:py-16">
            <div className="flex justify-center lg:justify-start">
              <Eyebrow />
            </div>

            <div className="mt-8 grid items-center gap-10 lg:grid-cols-[1.05fr_0.95fr] lg:gap-12">
              <div className="min-w-0 text-center lg:text-left">
                <h1 className="font-sans text-[34px] font-[700] leading-[0.9] tracking-[-0.04em] text-[#eceef0] sm:text-[44px] lg:text-[52px]">
                  <span className="block">Run everything.</span>
                  <span className="block text-[#ff4d00]">Wait for nothing.</span>
                </h1>
                <p className="mx-auto mt-4 max-w-[520px] text-[15px] leading-7 text-[#8a8f98] lg:mx-0">
                  One <code className="rounded-[6px] border border-[#1c1f22] bg-[#0f1113] px-1.5 py-0.5 font-mono text-[12px] text-[#c8ccd2]">fyrer.yml</code> for{' '}
                  <span className="font-medium text-[#eceef0]">any language, any stack</span>. Streaming DAG, blake3 cache — if it runs in a shell, fyrer orchestrates it.
                </p>

                <div className="mt-7 flex flex-col gap-3 sm:flex-row sm:justify-center lg:justify-start">
                  <Link to="/docs/quickstart" className="inline-flex h-[40px] items-center justify-center rounded-full bg-[#eceef0] px-6 text-[13.5px] font-[600] tracking-[-0.01em] text-[#08090a] transition hover:bg-white hover:translate-y-[-1px] no-underline" style={{textDecoration: 'none'}}>
                    Get Started
                  </Link>
                  <a href="https://github.com/07calc/fyrer" target="_blank" rel="noreferrer" className="inline-flex h-[40px] items-center justify-center gap-2 rounded-full border border-[#1c1f22] bg-[#0f1113] px-6 text-[13.5px] font-[500] text-[#eceef0] hover:border-[#252a2e] hover:bg-[#141718] no-underline" style={{textDecoration: 'none'}}>
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" /></svg>
                    View on GitHub
                  </a>
                </div>

                {/* single line install — less overwhelming than button + pills */}
                <div className="mt-4 flex items-center justify-center gap-2 font-mono text-[11px] text-[#5a5f68] lg:justify-start">
                  <span className="hidden sm:inline">$ {INSTALL_SHORT}</span>
                  <span className="sm:hidden">$ fyrer.sh</span>
                  <span className="h-3 w-px bg-[#1c1f22]" />
                  <span>Single binary • No daemon • MIT</span>
                </div>

                {/* lean stat line — cache miss vs hit */}
                <div className="mt-6 inline-flex flex-wrap items-center justify-center gap-2 rounded-full border border-[#1c1f22] bg-[#0f1113] px-4 py-2 font-mono text-[11px] lg:justify-start">
                  <span className="font-medium text-[#eceef0]">2.10s</span>
                  <span className="text-[#5a5f68]">cache miss</span>
                  <span className="h-1 w-1 rounded-full bg-[#1c1f22]" />
                  <span className="font-medium text-[#eceef0]">0.04s</span>
                  <span className="text-[#5a5f68]">cache hit</span>
                  <span className="h-1 w-1 rounded-full bg-[#1c1f22]" />
                  <span className="font-medium text-[#eceef0]">300ms</span>
                  <span className="text-[#5a5f68]">watch</span>
                </div>
              </div>

              <div className="mx-auto w-full max-w-[540px] lg:mx-0 lg:ml-auto">
                <HeroTerminal />
              </div>
            </div>

            <div className="mt-10 flex flex-col items-center justify-between gap-3 border-t border-[#141718] pt-6 sm:flex-row">
              <span className="font-mono text-[11px] tracking-[0.08em] text-[#5a5f68] uppercase">sh -c is the API — any language, any runtime</span>
              <span className="font-mono text-[11px] text-[#5a5f68]">No plugins • no adapters • just shell commands</span>
            </div>
          </div>
        </section>

        {/* 2 — PRODUCT — SIMPLIFIED */}
        <section className="border-b border-[#141718] bg-[#08090a]">
          <div className="mx-auto max-w-[1160px] px-4 py-12 sm:px-6 lg:py-16">
            <div className="mx-auto max-w-[640px] text-center">
              <SectionLabel k="02" label="Product" />
              <h2 className="mx-auto mt-4 max-w-[520px] font-sans text-[28px] font-[600] leading-[1.05] tracking-[-0.03em] text-[#eceef0] sm:text-[32px]">Your graph, streaming.</h2>
              <p className="mx-auto mt-3 max-w-[560px] text-[14px] leading-6 text-[#8a8f98]">
                Real <span className="text-[#c8ccd2]">fyrer</span> output from <code className="rounded border border-[#1c1f22] bg-[#0f1113] px-1.5 py-0.5 font-mono text-[12px] text-[#c8ccd2]">examples/acme-corp</code>. No waiting — tasks start the instant deps succeed.
              </p>
            </div>

            <div className="mt-10">
              <ProductViz />
            </div>
          </div>
        </section>

        {/* 3 — WORKFLOW — 3 COLS, MORE WHITESPACE */}
        <section className="border-b border-[#141718] bg-[#08090a]">
          <div className="mx-auto max-w-[1160px] px-4 py-12 sm:px-6 lg:py-16">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
              <div>
                <SectionLabel k="03" label="Workflow" />
                <h2 className="mt-4 max-w-[480px] font-sans text-[26px] font-[600] leading-[1.1] tracking-[-0.03em] text-[#eceef0] sm:text-[30px]">
                  Six primitives.
                  <br />
                  <span className="text-[#8a8f98]">No ceremony.</span>
                </h2>
              </div>
              <p className="max-w-[380px] font-mono text-[13px] leading-6 text-[#5a5f68] sm:text-right">Each card maps to a real fyrer.yml field. No marketing abstractions.</p>
            </div>

            <div className="mt-10 grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
              {FEATURES.map((f) => (
                <FeatureCard key={f.title} f={f} />
              ))}
            </div>
          </div>
        </section>

        {/* 4 — HOW IT WORKS — 3 STEPS */}
        <section className="border-b border-[#141718] bg-[#0f1113]">
          <div className="mx-auto max-w-[1160px] px-4 py-12 sm:px-6 lg:py-16">
            <SectionLabel k="04" label="How it works" />
            <div className="mt-4 grid gap-10 lg:grid-cols-[0.9fr_1.1fr] lg:items-start">
              <div>
                <h2 className="font-sans text-[26px] font-[600] leading-[1.1] tracking-[-0.03em] text-[#eceef0] sm:text-[30px]">From YAML to streaming.</h2>
                <p className="mt-3 max-w-[460px] text-[14px] leading-6 text-[#8a8f98]">
                  Every run parses fresh, validates, builds a DAG, hashes what matters, and streams tasks as soon as parents succeed.
                </p>

                <div className="mt-8 space-y-4">
                  {[
                    {n: '01', t: 'Parse & resolve DAG', d: 'Validate fyrer.yml, sort topologically, detect cycles. Plan is display-only.'},
                    {n: '02', t: 'Hash for cache', d: 'blake3(id + cmd + cwd + env + inputs). Outputs → .fyrer/cache/<key>.tar.zst'},
                    {n: '03', t: 'Stream execution', d: 'Up to concurrency, process groups, timeouts, SIGKILL on quit. Watch polls 300ms.'},
                  ].map((s) => (
                    <div key={s.n} className="flex gap-4 rounded-[12px] border border-[#1c1f22] bg-[#08090a] p-5">
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[8px] border border-[#1c1f22] bg-[#0f1113] font-mono text-[11px] font-medium text-[#ff4d00]">{s.n}</span>
                      <div>
                        <div className="font-sans text-[14px] font-[600] text-[#eceef0]">{s.t}</div>
                        <div className="mt-1 font-mono text-[12px] leading-5 text-[#5a5f68]">{s.d}</div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              <div className="min-w-0">
                <div className="overflow-hidden rounded-[14px] border border-[#1c1f22] bg-[#08090a]">
                  <div className="flex items-center gap-2 border-b border-[#1c1f22] px-3 py-2.5">
                    <span className="font-mono text-[11px] tracking-wide text-[#5a5f68]">architecture • dag → cache → run</span>
                    <span className="ml-auto rounded bg-[#1c1f22] px-2 py-0.5 font-mono text-[11px] text-[#5a5f68]">deterministic</span>
                  </div>
                  <div className="p-5">
                    <div className="grid grid-cols-3 items-center gap-2">
                      <div className="rounded-[10px] border border-[#ff4d00]/25 bg-[rgba(255,77,0,0.07)] px-3 py-3 text-center font-mono text-[11px] font-medium text-[#ff6b2b]">fyrer.yml</div>
                      <div className="text-center font-mono text-[#252a2e]">→</div>
                      <div className="rounded-[10px] border border-[#1c1f22] bg-[#0f1113] px-3 py-3 text-center font-mono text-[11px] text-[#8a8f98]">DAG</div>
                    </div>
                    <div className="my-3 flex justify-center font-mono text-[#252a2e]">↓</div>
                    {/* cache key — reduced */}
                    <div className="rounded-[12px] border border-[#1c1f22] bg-[#0f1113] p-4">
                      <div className="font-mono text-[11px] font-medium tracking-[0.08em] text-[#5a5f68] uppercase">Cache key — blake3</div>
                      <div className="mt-3 flex flex-wrap gap-2 font-mono text-[11px]">
                        {['id', 'cmd', 'env', 'inputs'].map((k) => (
                          <span key={k} className="rounded-[8px] border border-[#1c1f22] bg-[#08090a] px-3 py-2 text-[#8a8f98]">{k}</span>
                        ))}
                        <span className="rounded-full bg-[#ff4d00] px-3 py-1.5 text-white ml-auto">→ 32B</span>
                      </div>
                      <div className="mt-3 grid grid-cols-2 gap-2 font-mono text-[11px]">
                        <div className="rounded-[8px] border border-[rgba(16,185,129,0.18)] bg-[rgba(16,185,129,0.06)] px-3 py-2 text-[#10b981]">hit → ⚡ skip</div>
                        <div className="rounded-[8px] border border-[#1c1f22] bg-[#08090a] px-3 py-2 text-[#8a8f98]">miss → run & save</div>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="mt-4 flex flex-wrap gap-2">
                  <Link to="/docs/concepts/how-it-works" className="inline-flex rounded-full border border-[#1c1f22] bg-[#08090a] px-4 py-2 font-mono text-[12px] text-[#8a8f98] hover:text-[#eceef0] hover:border-[#252a2e] no-underline" style={{textDecoration: 'none'}}>
                    How it works →
                  </Link>
                  <Link to="/docs/concepts/caching" className="inline-flex rounded-full border border-[#1c1f22] bg-[#0f1113] px-4 py-2 font-mono text-[12px] text-[#8a8f98] hover:text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>
                    Caching
                  </Link>
                </div>
              </div>
            </div>
          </div>
        </section>

        {/* 5 — PERFORMANCE — SIMPLIFIED */}
        <section className="border-b border-[#141718] bg-[#08090a]">
          <div className="mx-auto max-w-[1160px] px-4 py-12 sm:px-6 lg:py-16">
            <div className="grid gap-10 lg:grid-cols-[0.95fr_1.05fr] lg:items-start">
              <div>
                <SectionLabel k="05" label="Performance" />
                <h2 className="mt-4 font-sans text-[26px] font-[600] leading-[1.1] tracking-[-0.03em] text-[#eceef0] sm:text-[30px]">
                  Second run is
                  <br />
                  <span className="text-[#ff4d00]">almost free.</span>
                </h2>
                <p className="mt-3 max-w-[440px] text-[14px] leading-6 text-[#8a8f98]">
                  Real local run. Same commit, no file changes — cache hit.
                </p>
                <div className="mt-8 grid grid-cols-2 gap-3">
                  {[
                    {v: '2.10s', l: 'cache miss'},
                    {v: '0.04s', l: 'cache hit'},
                  ].map((m) => (
                    <div key={m.l} className="rounded-[12px] border border-[#1c1f22] bg-[#0f1113] p-4 text-center">
                      <div className="font-mono text-[20px] font-[600] tracking-tight text-[#eceef0]">{m.v}</div>
                      <div className="font-mono text-[11px] tracking-wide text-[#5a5f68] uppercase">{m.l}</div>
                    </div>
                  ))}
                </div>
              </div>

              <div className="overflow-hidden rounded-[14px] border border-[#1c1f22] bg-[#0f1113]">
                <div className="flex items-center gap-2 border-b border-[#1c1f22] bg-[#08090a] px-3 py-2.5">
                  <span className="font-mono text-[11px] tracking-wide text-[#5a5f68]">benchmark • cache</span>
                  <span className="ml-auto rounded bg-[#1c1f22] px-2 py-0.5 font-mono text-[11px] text-[#8a8f98]">2 runs</span>
                </div>
                <div className="p-5">
                  <div className="space-y-4">
                    <div>
                      <div className="flex justify-between font-mono text-[11px]">
                        <span className="text-[#eceef0]">cache miss</span>
                        <span className="text-[#8a8f98]">2.10s</span>
                      </div>
                      <div className="mt-2 h-6 rounded-full bg-[#08090a] p-1">
                        <div className="h-full w-[70%] rounded-full bg-[#ff4d00]" />
                      </div>
                    </div>
                    <div>
                      <div className="flex justify-between font-mono text-[11px]">
                        <span className="text-[#10b981]">cache hit</span>
                        <span className="text-[#10b981]">0.04s</span>
                      </div>
                      <div className="mt-2 h-6 rounded-full bg-[#08090a] p-1">
                        <div className="h-full w-[4%] min-w-[8px] rounded-full bg-[#10b981]" />
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </section>

        {/* 6 — ONE FILE, EVERY LANGUAGE — FIXED */}
        <section className="border-b border-[#141718] bg-[#0f1113]">
          <div className="mx-auto max-w-[1160px] px-4 py-12 sm:px-6 lg:py-16">
            <div className="mx-auto max-w-[640px] text-center">
              <SectionLabel k="06" label="Polyglot" />
              <h2 className="mx-auto mt-4 font-sans text-[26px] font-[600] leading-[1.1] tracking-[-0.03em] text-[#eceef0] sm:text-[30px]">One file. Every language.</h2>
              <p className="mx-auto mt-3 max-w-[560px] text-[14px] leading-6 text-[#8a8f98]">A single fyrer.yml declares tasks for every runtime. No plugins — just shell commands.</p>
            </div>

            <div className="mt-10 grid gap-6 lg:grid-cols-[1.15fr_0.85fr] lg:items-start">
              {/* left — clean yaml */}
              <CodePanel
                title="fyrer.yml"
                lang="YAML"
                meta="one file • any stack"
                code={`version: 1
packages:
  - name: ui
    tasks:
      build: { cmd: bun build src --outdir dist }

  - name: api
    tasks:
      build: { cmd: cargo build --release }

  - name: worker
    tasks:
      build: { cmd: go build -o bin/worker . }

  - name: jobs
    tasks:
      run: { cmd: python3 jobs/run.py }
  # any command via sh -c — if it runs in a shell, fyrer runs it`}
              />

              {/* right — visual any-runtime + commands */}
              <div className="space-y-4">
                <div className="rounded-[14px] border border-[#1c1f22] bg-[#08090a] p-4">
                  <div className="font-mono text-[11px] font-medium tracking-[0.08em] text-[#5a5f68] uppercase">Any runtime • any toolchain</div>
                  <div className="mt-3 flex flex-wrap gap-2 font-mono text-[11px]">
                    <span className="rounded-full border border-[#1c1f22] bg-[#0f1113] px-3 py-1.5 text-[#8a8f98]">bun build</span>
                    <span className="rounded-full border border-[#1c1f22] bg-[#0f1113] px-3 py-1.5 text-[#8a8f98]">cargo build</span>
                    <span className="rounded-full border border-[#1c1f22] bg-[#0f1113] px-3 py-1.5 text-[#8a8f98]">go build</span>
                    <span className="rounded-full border border-[#1c1f22] bg-[#0f1113] px-3 py-1.5 text-[#8a8f98]">python3</span>
                    <span className="rounded-full border border-[#1c1f22] bg-[#0f1113] px-3 py-1.5 text-[#8a8f98]">make</span>
                    <span className="rounded-full border border-[#1c1f22] bg-[#0f1113] px-3 py-1.5 text-[#8a8f98]">./scripts/*</span>
                    <span className="rounded-full border border-[#ff4d00]/20 bg-[rgba(255,77,0,0.06)] px-3 py-1.5 text-[#ff6b2b]">… sh -c</span>
                  </div>
                  <div className="mt-4 flex items-center gap-2 rounded-[10px] border border-[#1c1f22] bg-[#0f1113] px-3 py-2.5 font-mono text-[11px] text-[#5a5f68]">
                    <span className="h-1.5 w-1.5 rounded-full bg-[#10b981]" />
                    Same file also handles <span className="text-[#8a8f98]">depends_on • inputs • cache • watch</span>
                  </div>
                </div>

                <CodePanel
                  title="run it"
                  lang="bash"
                  meta="—n for CI"
                  code={`$ fyrer run build -n
[ui:build]      ✓ 310ms
[worker:build]  ✓ 1.14s
[api:build]     ✓ 2.10s
Run 2.10s  ✓3  ⚡0  ↷0`}
                />

                <div className="flex flex-wrap gap-2">
                  <span className="rounded-full border border-[#1c1f22] bg-[#08090a] px-3 py-1.5 font-mono text-[11px] text-[#8a8f98]">sh -c • any command</span>
                  <span className="rounded-full border border-[#1c1f22] bg-[#08090a] px-3 py-1.5 font-mono text-[11px] text-[#8a8f98]">no wrappers</span>
                  <span className="rounded-full border border-[#1c1f22] bg-[#08090a] px-3 py-1.5 font-mono text-[11px] text-[#8a8f98]">no daemons</span>
                </div>
              </div>
            </div>
          </div>
        </section>

        {/* 7 — COMPARISON */}
        <section className="border-b border-[#141718] bg-[#08090a]">
          <div className="mx-auto max-w-[1160px] px-4 py-12 sm:px-6 lg:py-16">
            <div className="grid gap-10 lg:grid-cols-[0.9fr_1.1fr]">
              <div>
                <SectionLabel k="07" label="Comparison" />
                <h2 className="mt-4 font-sans text-[26px] font-[600] leading-[1.1] tracking-[-0.03em] text-[#eceef0] sm:text-[28px]">
                  Streaming execution.
                </h2>
                <p className="mt-3 max-w-[440px] text-[14px] leading-6 text-[#8a8f98]">Every task starts the moment its dependencies succeed — maximum parallelism, zero idle waits.</p>
                <div className="mt-6 rounded-[12px] border border-[#1c1f22] bg-[#0f1113] p-4">
                  <div className="font-mono text-[11px] tracking-[0.08em] text-[#5a5f68] uppercase">Migrate</div>
                  <div className="mt-3 font-mono text-[12px] leading-6 text-[#8a8f98]">
                    <div>
                      <span className="text-[#5a5f68]">$</span> {INSTALL}
                    </div>
                    <div className="text-[#5a5f68]"># then copy turbo.json → fyrer.yml</div>
                  </div>
                  <div className="mt-4 flex gap-2">
                    <Link to="/docs/quickstart" className="rounded-full bg-[#eceef0] px-4 py-2 font-mono text-[11px] font-[600] text-[#08090a] no-underline" style={{textDecoration: 'none'}}>
                      Quickstart
                    </Link>
                  </div>
                </div>
              </div>

              <div className="overflow-hidden rounded-[14px] border border-[#1c1f22] bg-[#0f1113]">
                <div className="grid grid-cols-[1fr_140px] gap-px bg-[#1c1f22] font-mono text-[11px]">
                  <div className="bg-[#0f1113] px-3 py-2.5 font-medium tracking-wide text-[#5a5f68] uppercase">Capability</div>
                  <div className="bg-[#0f1113] px-3 py-2.5 text-center font-medium tracking-wide text-[#ff4d00]">fyrer</div>
                  {[
                    {k: 'Execution', b: 'streaming DAG'},
                    {k: 'Cache', b: 'blake3 • content-addressed'},
                    {k: 'Languages', b: 'any • sh -c'},
                    {k: 'Runtime', b: 'single binary • no daemon'},
                  ].map((r) => (
                    <React.Fragment key={r.k}>
                      <div className="bg-[#0f1113] px-3 py-3 text-[#c8ccd2]">{r.k}</div>
                      <div className="bg-[#0f1113] px-3 py-3 text-center font-medium text-[#10b981]">{r.b}</div>
                    </React.Fragment>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </section>

        {/* 8 — OPEN SOURCE */}
        <section className="border-b border-[#141718] bg-[#0f1113]">
          <div className="mx-auto max-w-[1160px] px-4 py-12 sm:px-6 lg:py-16">
            <div className="grid gap-10 lg:grid-cols-[1fr_1fr]">
              <div>
                <SectionLabel k="08" label="Open source" />
                <h2 className="mt-4 font-sans text-[26px] font-[600] tracking-tight text-[#eceef0] sm:text-[28px]">Built in the open.</h2>
                <p className="mt-3 max-w-[460px] text-[14px] leading-6 text-[#8a8f98]">MIT, Rust, no telemetry. Install via sh or cargo.</p>
                <div className="mt-6 grid grid-cols-3 gap-3">
                  {[
                    {k: 'License', v: 'MIT'},
                    {k: 'Lang', v: 'Rust'},
                    {k: 'Telemetry', v: 'None'},
                  ].map((x) => (
                    <div key={x.k} className="rounded-[12px] border border-[#1c1f22] bg-[#08090a] px-3 py-3 text-center">
                      <div className="font-mono text-[11px] tracking-wide text-[#5a5f68] uppercase">{x.k}</div>
                      <div className="mt-1 font-mono text-[13px] font-medium text-[#eceef0]">{x.v}</div>
                    </div>
                  ))}
                </div>
                <div className="mt-5 flex flex-wrap gap-2">
                  <a href="https://github.com/07calc/fyrer" className="inline-flex items-center gap-2 rounded-full bg-[#eceef0] px-4 py-2 font-mono text-[12px] font-[600] text-[#08090a] no-underline" style={{textDecoration: 'none'}}>
                    <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" /></svg>
                    Star
                  </a>
                  <a href="https://github.com/07calc/fyrer/issues" className="rounded-full border border-[#1c1f22] bg-[#08090a] px-4 py-2 font-mono text-[12px] text-[#8a8f98] no-underline" style={{textDecoration: 'none'}}>Issues</a>
                  <a href="https://github.com/07calc/fyrer/releases" className="rounded-full border border-[#1c1f22] bg-[#08090a] px-4 py-2 font-mono text-[12px] text-[#8a8f98] no-underline" style={{textDecoration: 'none'}}>Releases</a>
                </div>
              </div>

              <div className="overflow-hidden rounded-[14px] border border-[#1c1f22] bg-[#08090a]">
                <div className="flex items-center gap-2 border-b border-[#1c1f22] px-3 py-2.5">
                  <span className="font-mono text-[11px] tracking-wide text-[#5a5f68]">install — any platform</span>
                  <span className="ml-auto font-mono text-[11px] text-[#5a5f68]">6 targets</span>
                </div>
                <div className="p-4 space-y-3">
                  <div className="rounded-[10px] border border-[#1c1f22] bg-[#0f1113] p-3">
                    <div className="flex items-center justify-between">
                      <span className="font-mono text-[11px] tracking-wide text-[#5a5f68] uppercase">Unix</span>
                      <CopyBtn text={INSTALL} small />
                    </div>
                    <pre className="mt-2 overflow-x-auto font-mono text-[12px] text-[#c8ccd2]">{INSTALL}</pre>
                  </div>
                  <div className="rounded-[10px] border border-[#1c1f22] bg-[#0f1113] p-3">
                    <div className="flex items-center justify-between">
                      <span className="font-mono text-[11px] tracking-wide text-[#5a5f68] uppercase">Cargo</span>
                      <CopyBtn text="cargo install fyrer" small />
                    </div>
                    <pre className="mt-2 font-mono text-[12px] text-[#c8ccd2]">cargo install fyrer</pre>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </section>

        {/* 9 — FINAL CTA */}
        <section className="relative overflow-hidden border-b border-[#141718] bg-[#08090a]">
          <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(700px_circle_at_50%_0px,rgba(255,77,0,0.06),transparent_70%)]" />
          <div className="relative mx-auto max-w-[1160px] px-4 py-12 sm:px-6 lg:py-16">
            <div className="overflow-hidden rounded-[16px] border border-[#1c1f22] bg-[#0f1113] p-6 sm:p-8 lg:p-10">
              <div className="grid gap-8 lg:grid-cols-[1.1fr_0.9fr] lg:items-center">
                <div>
                  <h2 className="font-sans text-[28px] font-[700] leading-[1.05] tracking-[-0.03em] text-[#eceef0] sm:text-[34px]">Stop waiting.</h2>
                  <p className="mt-3 max-w-[480px] text-[14px] leading-6 text-[#8a8f98]">
                    One <code className="rounded border border-[#1c1f22] bg-[#08090a] px-1.5 py-0.5 font-mono text-[12px] text-[#c8ccd2]">fyrer.yml</code>. Every language. <span className="text-[#eceef0]">0.04s</span> on cache hit.
                  </p>
                  <div className="mt-6 flex flex-wrap gap-3">
                    <Link to="/docs/quickstart" className="inline-flex h-10 items-center justify-center rounded-full bg-[#eceef0] px-6 text-[13.5px] font-[600] text-[#08090a] no-underline" style={{textDecoration: 'none'}}>
                      Get Started →
                    </Link>
                    <a href="https://github.com/07calc/fyrer" className="inline-flex h-10 items-center justify-center rounded-full border border-[#1c1f22] bg-[#08090a] px-6 text-[13.5px] font-[500] text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>
                      View on GitHub
                    </a>
                  </div>
                </div>

                <div className="overflow-hidden rounded-[12px] border border-[#1c1f22] bg-[#08090a]">
                  <div className="flex items-center gap-2 border-b border-[#1c1f22] px-3 py-2.5">
                    <span className="font-mono text-[11px] tracking-wide text-[#5a5f68]">$ install & run</span>
                  </div>
                  <div className="p-4 space-y-3">
                    <div className="flex items-center gap-2 rounded-[10px] border border-[#1c1f22] bg-[#0f1113] px-3 py-3 font-mono text-[12px]">
                      <span className="text-[#5a5f68]">$</span>
                      <span className="flex-1 truncate text-[#eceef0]">{INSTALL}</span>
                      <CopyBtn text={INSTALL} small />
                    </div>
                    <div className="grid grid-cols-2 gap-2 font-mono text-[12px]">
                      <div className="rounded-[10px] border border-[#1c1f22] bg-[#0f1113] px-3 py-2.5">
                        <span className="text-[#5a5f68]">$</span> <span className="text-[#c8ccd2]">fyrer run build</span>
                        <div className="text-[11px] text-[#10b981]">2.10s → 0.04s</div>
                      </div>
                      <div className="rounded-[10px] border border-[#1c1f22] bg-[#0f1113] px-3 py-2.5">
                        <span className="text-[#5a5f68]">$</span> <span className="text-[#c8ccd2]">fyrer run dev</span>
                        <div className="text-[11px] text-[#8a8f98]">q to quit</div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </section>

      </div>
      <style>{`@keyframes blink { 0%, 49% { opacity: 1 } 50%, 100% { opacity: 0 } }`}</style>
    </Layout>
  );
}
