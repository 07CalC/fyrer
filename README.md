# fyrer 

lightweight tool to run multiple dev servers concurrently

## Installation

### install using cargo:
  
```bash
cargo install fyrer
```

### build from source:

```bash
git clone https://github.com/07calc/fyrer
cd fyrer
cargo build --release
cargo install --path .
```

## Usage

`fyrer` looks for a `fyrer.yml` file in the current directory:

```bash
fyrer
```

example config file `fyrer.yml`:

```yaml
installers:
  - dir: ./project1
    cmd: pip install -r requirements.txt

services:
  - name: server1
    cmd: python -m http.server 8000
    dir: ./project1
    env_path: .env.local  ## .env file path
    env:                  ## overrides the .env file
      PORT: 8000
      ENV: dev

  - name: server2
    cmd: npm start
    dir: ./project2
    watch: true  # enable hot reload
    ignore:
      - "node_modules/**"
      - "*.db"
```

## Features

- Run multiple development servers concurrently
- Define installer commands that run before starting each server
- Set a working directory per server
- Automatic env parsing from .env file
- Assign environment variables per server (overrides the env file)
- YAML-based configuration file
- Prefixed log output for readability
- Cross-platform support (Linux, macOS, Windows)
- Optional hot reload
- Configurable file and directory ignore rules for hot reload

## Notes

- `watch: true` enables file monitoring for that server.
- Ignore patterns follow `glob` syntax.
- restarts servers when watched files change.
- envs defined in `fyrer.yml` overrides those in `.env` file.
