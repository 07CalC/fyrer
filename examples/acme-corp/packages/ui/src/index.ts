export const brand = "acme-corp";

export function renderHome(): string {
  return `<h1>acme-corp UI</h1><p>Rendered by <code>@acme/ui</code></p>`;
}

export function renderCard(title: string, body: string): string {
  return `<div style="border:1px solid #ccc;padding:8px;margin:8px 0"><h2>${title}</h2><p>${body}</p></div>`;
}