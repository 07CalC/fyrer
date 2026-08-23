import React from 'react';
import Link from '@docusaurus/Link';

export default function Footer(): React.ReactNode {
  return (
    <footer className="bg-[#08090a]">
      <div className="mx-auto max-w-[1160px] px-4 py-8 sm:px-6">
        <div className="flex flex-col gap-8 sm:flex-row sm:justify-between">
          <div>
            <div className="flex items-center gap-2.5">
              <span className="flex h-7 w-7 items-center justify-center rounded-[8px] border border-[#1c1f22] bg-[#0f1113] font-mono text-[12px] font-[600] text-[#eceef0]">f</span>
              <span className="font-sans text-[14px] font-[600] tracking-tight text-[#eceef0]">fyrer</span>
              <span className="rounded-full border border-[#1c1f22] bg-[#0f1113] px-2 py-0.5 font-mono text-[11px] text-[#5a5f68]">v0.5.0</span>
            </div>
            <div className="mt-2 max-w-[420px] font-mono text-[12px] leading-5 text-[#5a5f68]">Streaming DAG for polyglot monorepos. Single binary, no daemon.</div>
          </div>

          <div className="grid grid-cols-3 gap-8 font-mono text-[12px] sm:gap-10">
            <div>
              <div className="font-medium tracking-[0.08em] text-[#5a5f68] uppercase">Docs</div>
              <div className="mt-3 space-y-2">
                <Link to="/docs/introduction" className="block text-[#8a8f98] hover:text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>Introduction</Link>
                <Link to="/docs/quickstart" className="block text-[#8a8f98] hover:text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>Quickstart</Link>
                <Link to="/docs/configuration/overview" className="block text-[#8a8f98] hover:text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>Configuration</Link>
              </div>
            </div>
            <div>
              <div className="font-medium tracking-[0.08em] text-[#5a5f68] uppercase">Community</div>
              <div className="mt-3 space-y-2">
                <a href="https://github.com/07calc/fyrer" className="block text-[#8a8f98] hover:text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>GitHub</a>
                <a href="https://github.com/07calc/fyrer/issues" className="block text-[#8a8f98] hover:text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>Issues</a>
                <a href="https://github.com/07calc/fyrer/discussions" className="block text-[#8a8f98] hover:text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>Discussions</a>
              </div>
            </div>
            <div>
              <div className="font-medium tracking-[0.08em] text-[#5a5f68] uppercase">More</div>
              <div className="mt-3 space-y-2">
                <a href="https://github.com/07calc/fyrer/releases" className="block text-[#8a8f98] hover:text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>Releases</a>
                <a href="https://github.com/07calc/fyrer/blob/main/LICENSE" className="block text-[#8a8f98] hover:text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>License</a>
              </div>
            </div>
          </div>
        </div>

        <div className="mt-8 flex flex-col gap-3 border-t border-[#141718] pt-6 sm:flex-row sm:justify-between">
          <span className="font-mono text-[11px] tracking-wide text-[#5a5f68]">© {new Date().getFullYear()} fyrer • MIT • single binary • 6 targets</span>
          <span className="font-mono text-[11px] text-[#5a5f68]">
            <a href="https://github.com/07calc/fyrer" className="text-[#8a8f98] hover:text-[#eceef0] no-underline" style={{textDecoration: 'none'}}>github.com/07calc/fyrer</a>
          </span>
        </div>
      </div>
    </footer>
  );
}
