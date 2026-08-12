# acme-corp — fyrer demo monorepo

A fictional monorepo that exercises every feature of
[fyrer](https://github.com/07calc/fyrer). It mixes four languages
across apps and shared packages, wired together with `depends_on` edges so the
execution plan is easy to see.

## Layout

```
acme-corp/
├── fyrer.yml               # the demo configuration
├── apps/
│   ├── web/                # TypeScript / Bun  — frontend dev server
│   ├── api/                # Rust  / Cargo    — HTTP API
│   ├── worker/             # Go               — HTTP worker service
│   └── cli/                # Python           — demo CLI (bundle, lint, serve, docs)
└── packages/
    ├── ui/                 # TypeScript / Bun  — shared UI package (built with bun)
    ├── shared/             # Rust  / Cargo    — shared crate (path dependency of api)
    └── core/               # Go               — shared package (replace'd into worker)
```

## Config features on show

- **Root-level `env`** shared by every task (`NODE_ENV`, `LOG_LEVEL`).
- **Per-package `env`** and **`env_file`** (`.env` parsing) with documented
  precedence: root env < package env_file < package env < task env_file < task env.
  See `apps/api/.env` (PORT/RUST_LOG), `packages/ui/.env` and task-level `env`
  overrides on `web:build`/`ui:watch`/`cli:serve`.
- **Dev servers** that run concurrently until you hit Ctrl+C:
  `web:dev`, `api:dev`, `worker:dev` (all `persistent: true`), plus
  `cli:serve` and the `ui:watch` watch-build.
- **Cached builds**: every `build`/`bundle`/`lint`/`docs` task is `cache: true`
  with `inputs` + `outputs` globs and `ignore` exclusions. Cache entries live
  under `.fyrer/cache/`.
- **`depends_on` across packages**: `web:build -> ui:build`, `api:build ->
  shared:build`, `worker:build -> core:build`, `cli:serve -> cli:bundle`.
- **`cwd`** support: `cli:docs` runs its command from `scripts/`.
- **`timeout`**: `cli:timeout-demo` is killed 2s in.
- **`watch` / `persistent`** flags are declared on long-running tasks.

> Note: in fyrer 0.3.0 the `watch`/`persistent` flags are part of the config
> schema and shown by `list`/`plan`, but the auto-restart watcher is not yet
> wired into the executor.

## Demo

Run everything from this directory, because package roots are relative to the
current working directory:

```bash
cd examples/acme-corp
```

Inspect the repo and the execution plan:

```bash
fyrer list
fyrer plan build
```

Build everything (first run compiles, second run is served from cache):

```bash
fyrer run build
fyrer run build   # ⚡ Cached
```

Boot all the dev servers (web + api + worker, plus the `ui:build` they depend
on). They stream logs; press Ctrl+C to stop:

```bash
fyrer run dev
```

Individual pieces:

```bash
fyrer run web:dev            # a single server
fyrer run ui:watch           # watch-mode build of @acme/ui
fyrer run cli:serve          # python http.server, PORT overridden by task env
fyrer run cli:timeout-demo   # killed after 2s by the timeout option
fyrer run bundle             # cli bundle + lint + docs, cached
fyrer run cli:docs           # docs generated from scripts/ via the cwd option
```

## Requirements

- [bun](https://bun.sh) for `ui`/`web`
- [cargo](https://rustup.rs) for `shared`/`api`
- [go](https://go.dev) for `core`/`worker`
- [python3](https://www.python.org) for `cli`

Everything is dependency-free stdlib-only code, so no `install` step and no
network access are needed.