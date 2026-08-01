# fyrer

A declarative, fast and lightweight monorepo tool that runs multiple dev
servers and build tasks concurrently.

`fyrer` reads a `fyrer.yml` file describing projects and their tasks, resolves
the dependency graph between tasks, runs each level of the graph concurrently,
and streams every task's output through a colorized, prefixed logger.
Long-running tasks can be restarted automatically when their watched input
files change.

## Installation

### Install using cargo

```bash
cargo install fyrer
```

### Build from source

```bash
git clone https://github.com/07calc/fyrer
cd fyrer
cargo build --release
cargo install --path .
```

## Usage

`fyrer` looks for a `fyrer.yml` file in the current directory. A full example
is available at [`fyrer.example.yml`](./fyrer.example.yml).

```bash
# Run every task
fyrer run

# Run a single task or every task with a given name
fyrer run web:dev
fyrer run build

# Show the execution plan without running anything
fyrer run dev --dry-run

# List every project and its tasks
fyrer list

# Point at a different configuration file
fyrer --config path/to/fyrer.yml run
```

## Configuration

```yaml
version: 1                       # config format version (required)
env:                             # variables shared by every project
  NODE_ENV: development
projects:                        # projects that make up the monorepo
  - name: web                    # unique project name
    root: ./apps/web             # relative project root
    env:                         # variables shared by every task
      PORT: "3000"
    env_path: .env               # .env file path, relative to root
    tasks:
      dev:
        cmd: bun run dev         # shell command to run
        depends_on: [build]      # tasks that must run first
        inputs: [src/**]         # globs of watched input files
        outputs: [dist/**]       # globs of produced output files
        ignore: [node_modules/**]  # globs excluded from watching
        cache: false             # allow skipping when outputs are fresh
        restart:
          strategy: FileChange   # FileChange | OnFailure | Never
          delay: 300             # debounce before restarting (ms)
        env:                     # per-task variables (highest precedence)
          PORT: "8080"
```

### Environment precedence

From lowest to highest: root-level `env`, project-level `env`, the project's
`.env` file, then task-level `env`.

### Restart strategies

- `FileChange` – restart whenever a watched input file changes (requires
  `inputs`).
- `OnFailure` – restart after the process exits with a failure.
- `Never` – never restart.

## Features

- Run multiple development servers and build tasks concurrently
- Declarative YAML-based configuration with validation
- Dependency-aware topological ordering
- Automatic `.env` file parsing with per-task overrides
- Colorized, prefixed log output
- Optional file-watching with automatic restarts and debouncing
- Graceful shutdown on `SIGINT`/`SIGTERM`, killing spawned process groups

## License

MIT
