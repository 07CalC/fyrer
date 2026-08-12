import { brand, renderHome, renderCard } from "../../../packages/ui/dist/index.js";

const appName = process.env.APP_NAME ?? "web";
const port = Number(process.env.PORT ?? 3000);
const apiUrl = process.env.API_URL ?? "http://localhost:8080";
const nodeEnv = process.env.NODE_ENV ?? "production";

const server = Bun.serve({
  port,
  fetch() {
    const html = `<!doctype html>
<html>
  <body style="font-family:system-ui">
    ${renderHome()}
    ${renderCard("App", appName)}
    ${renderCard("API", apiUrl)}
    ${renderCard("Environment", `${nodeEnv} (NODE_ENV) `)}
    <p>ui package: <code>${brand}</code></p>
  </body>
</html>`;
    return new Response(html, {
      headers: { "content-type": "text/html" },
    });
  },
});

console.log(
  `[web] listening on http://localhost:${server.port} app=${appName} api=${apiUrl} env=${nodeEnv}`,
);