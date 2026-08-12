declare const Bun: {
  serve(options: {
    port: number | string;
    fetch(req?: Request, server?: unknown): Response | Promise<Response>;
  }): { port: number };
};

declare const process: {
  env: Record<string, string | undefined>;
};

declare module "../../../packages/ui/dist/index.js" {
  export const brand: string;
  export function renderHome(): string;
  export function renderCard(title: string, body: string): string;
}