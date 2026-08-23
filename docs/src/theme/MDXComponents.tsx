import React from 'react';
import MDXComponents from '@theme-original/MDXComponents';
import DocusaurusTabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

type CardProps = {title?: string; icon?: string; href?: string; children: React.ReactNode};
type CardGroupProps = {cols?: number; children: React.ReactNode};
type StepsProps = {children: React.ReactNode};
type StepProps = {title?: string; children: React.ReactNode};
type TabsProps = {children: React.ReactNode; groupId?: string};
type TabProps = {title?: string; label?: string; value?: string; children: React.ReactNode};
type ColumnsProps = {children: React.ReactNode};
type CalloutProps = {children: React.ReactNode; title?: string};
type ParamFieldProps = {children: React.ReactNode; body?: string; path?: string; type?: string; required?: boolean; default?: string};

const iconMap: Record<string, string> = {
  download: '⬇',
  rocket: '🚀',
  'book-open-cover': '📖',
  sliders: '🎛',
  bolt: '⚡',
  'layer-group': '◈',
  'diagram-project': '⬢',
  database: '⧉',
  server: '▣',
  gears: '⚙',
  code: '›_',
  folder: '📁',
  'list-check': '☑',
  key: '🔑',
};

function Card({title, icon, href, children}: CardProps) {
  const resolvedIcon = icon ? iconMap[icon] ?? icon : null;
  const content = (
    <div className="group relative flex h-full flex-col rounded-[12px] border border-[#1c1f22] bg-[#0f1113] p-4 transition-all duration-200 hover:border-[#252a2e] hover:bg-[#141718] sm:p-5">
      <div className="flex items-start justify-between gap-2 sm:gap-3">
        <div className="flex items-center gap-2 sm:gap-3">
          {resolvedIcon && (
            <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[8px] border border-[#1c1f22] bg-[#08090a] font-mono text-xs text-[#8a8f98] group-hover:border-[#252a2e] group-hover:text-[#eceef0] sm:h-8 sm:w-8 sm:text-sm">
              {resolvedIcon}
            </span>
          )}
          {title && <div className="text-[14px] font-semibold leading-tight tracking-tight text-[#eceef0] sm:text-[15px]">{title}</div>}
        </div>
        {href && <span className="shrink-0 font-mono text-[#5a5f68] transition group-hover:translate-x-0.5 group-hover:text-[#ff4d00]">→</span>}
      </div>
      <div className="mt-2 text-sm leading-6 text-[#8a8f98] sm:mt-2.5">{children}</div>
    </div>
  );
  if (href) {
    const external = href.startsWith('http');
    return (
      <a
        href={href}
        target={external ? '_blank' : undefined}
        rel={external ? 'noopener noreferrer' : undefined}
        className="block h-full no-underline"
        style={{textDecoration: 'none', color: 'inherit'}}
      >
        {content}
      </a>
    );
  }
  return content;
}

function CardGroup({cols = 2, children}: CardGroupProps) {
  const count = React.Children.toArray(children).filter(Boolean).length;
  const eff = Math.min(cols, count || cols);
  const cls =
    eff === 1
      ? 'grid-cols-1'
      : eff === 3
        ? 'grid-cols-1 sm:grid-cols-2 lg:grid-cols-3'
        : eff >= 4
          ? 'grid-cols-1 sm:grid-cols-2 lg:grid-cols-4'
          : 'grid-cols-1 sm:grid-cols-2';
  return <div className={`my-6 grid gap-4 ${cls}`}>{children}</div>;
}

function Steps({children}: StepsProps) {
  const items = React.Children.toArray(children).filter(Boolean);
  return (
    <div className="relative my-6 ml-2 border-l border-[#1c1f22] pl-6 sm:my-8 sm:ml-3 sm:pl-8">
      {items.map((child, idx) => (
        <div key={idx} className="relative pb-6 last:pb-0 sm:pb-8">
          <span className="absolute -left-[25px] flex h-6 w-6 items-center justify-center rounded-full border border-[#1c1f22] bg-[#eceef0] text-[11px] font-bold text-[#08090a] shadow-sm sm:-left-[37px] sm:h-7 sm:w-7 sm:text-xs">
            {idx + 1}
          </span>
          <div className="pt-0 sm:pt-0.5">{child as React.ReactNode}</div>
        </div>
      ))}
    </div>
  );
}

function Step({title, children}: StepProps) {
  return (
    <div>
      {title && <div className="text-[15px] font-semibold tracking-tight text-[#eceef0]">{title}</div>}
      <div className="prose prose-sm mt-2 max-w-none text-[14.5px] leading-7 text-[#a8adb5] [&_p]:my-3 [&_ul]:my-3 [&_pre]:my-4">{children}</div>
    </div>
  );
}

function Tabs({children, groupId}: TabsProps) {
  const items = React.Children.toArray(children).filter(Boolean) as React.ReactElement[];
  const values = items.map((c, idx) => {
    const p: any = c.props || {};
    const raw = p.title ?? p.label ?? p.value ?? `tab-${idx}`;
    const base = String(raw).toLowerCase().trim().replace(/\s+/g, '-').replace(/[^a-z0-9-_]/g, '') || `tab-${idx}`;
    return {label: String(raw), value: `${base}-${idx}`};
  });
  const normalized = items.map((c, idx) => {
    const v = values[idx]?.value;
    return React.cloneElement(c as React.ReactElement<any>, {value: v, label: values[idx].label, key: v});
  });
  return (
    <DocusaurusTabs groupId={groupId} values={values}>
      {normalized}
    </DocusaurusTabs>
  );
}
function Tab({title, label, value, children, ...props}: TabProps) {
  const raw = value ?? title ?? label ?? 'tab';
  const sanitized = String(raw).toLowerCase().trim().replace(/\s+/g, '-').replace(/[^a-z0-9-_]/g, '') || 'tab';
  const v = sanitized;
  const l = label ?? title ?? String(value ?? 'Tab');
  return (
    <TabItem value={v} label={l} {...props}>
      {children}
    </TabItem>
  );
}

function Columns({children}: ColumnsProps) {
  return <div className="my-6 grid grid-cols-1 gap-4 md:grid-cols-2">{children}</div>;
}

function Note({children, title}: CalloutProps) {
  return (
    <div className="my-4 flex gap-2.5 rounded-[12px] border border-[#1c1f22] bg-[#0f1113] p-3 sm:my-5 sm:gap-3 sm:p-4">
      <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[#eceef0] font-mono text-xs font-bold text-[#08090a] sm:h-7 sm:w-7 sm:text-sm">i</span>
      <div className="min-w-0 flex-1">
        {title && <div className="text-sm font-semibold text-[#eceef0]">{title}</div>}
        <div className="text-[13px] leading-6 text-[#a8adb5] sm:text-sm">{children}</div>
      </div>
    </div>
  );
}
function Tip({children, title}: CalloutProps) {
  return (
    <div className="my-4 flex gap-2.5 rounded-[12px] border border-[rgba(16,185,129,0.14)] bg-[rgba(16,185,129,0.06)] p-3 sm:my-5 sm:gap-3 sm:p-4">
      <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[#10b981] font-mono text-xs font-bold text-white sm:h-7 sm:w-7 sm:text-sm">✓</span>
      <div className="min-w-0 flex-1">
        {title && <div className="text-sm font-semibold text-[#a7f3d0]">{title}</div>}
        <div className="text-[13px] leading-6 text-[#a7f3d0]/80 sm:text-sm">{children}</div>
      </div>
    </div>
  );
}
function Warning({children, title}: CalloutProps) {
  return (
    <div className="my-4 flex gap-2.5 rounded-[12px] border border-[rgba(245,158,11,0.14)] bg-[rgba(245,158,11,0.06)] p-3 sm:my-5 sm:gap-3 sm:p-4">
      <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[#f59e0b] font-mono text-xs font-bold text-white sm:h-7 sm:w-7 sm:text-sm">!</span>
      <div className="min-w-0 flex-1">
        {title && <div className="text-sm font-semibold text-[#fde68a]">{title}</div>}
        <div className="text-[13px] leading-6 text-[#fde68a]/80 sm:text-sm">{children}</div>
      </div>
    </div>
  );
}

function ParamField({children, body, type, required, path, default: def}: ParamFieldProps) {
  const name = path || body || '';
  return (
    <div className="my-4 overflow-hidden rounded-[12px] border border-[#1c1f22] bg-[#0f1113]">
      <div className="flex flex-wrap items-center gap-2 border-b border-[#1c1f22] bg-[#08090a] px-3 py-3 sm:px-4">
        <code className="rounded-md border border-[#1c1f22] bg-[#0f1113] px-2 py-1 font-mono text-[13px] font-semibold text-[#eceef0] sm:text-sm">{name}</code>
        {type && (
          <span className="rounded-full bg-[#eceef0] px-2 py-1 font-mono text-[11px] font-medium text-[#08090a] sm:px-2.5 sm:text-xs">{type}</span>
        )}
        {required && <span className="rounded-full bg-[#ff4d00] px-2 py-1 text-[11px] font-bold text-white sm:px-2.5 sm:text-xs">required</span>}
        {def && <span className="w-full font-mono text-xs text-[#5a5f68] sm:ml-auto sm:w-auto">default: {def}</span>}
      </div>
      <div className="px-3 py-3 text-sm leading-6 text-[#8a8f98] sm:px-4">{children}</div>
    </div>
  );
}

export default {
  ...MDXComponents,
  Card,
  CardGroup,
  Steps,
  Step,
  Tabs,
  Tab,
  Columns,
  Note,
  Tip,
  Warning,
  ParamField,
};
