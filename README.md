# fyrer 

lightweight tool to run multiple dev servers concurrently

## Features

- Run multiple development servers concurrently
- Define installer commands that run before starting each server
- Set a working directory per server
- Assign environment variables per server
- YAML-based configuration file
- Prefixed log output for readability
- Cross-platform support (Linux, macOS, Windows)
- Optional hot reload
- Configurable file and directory ignore rules for hot reload

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
    env:
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

## Notes

- `watch: true` enables file monitoring for that server.
- Ignore patterns follow `glob` syntax.
- restarts servers when watched files change.
