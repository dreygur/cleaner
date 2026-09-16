# cleaner

A disk reclaimer for macOS development machines. It finds build output, package
caches, and downloaded dependencies (the things a tool can rebuild), measures
them, and deletes only what you approve.

It does not touch source code, browser profiles, container volumes, or anything
outside your home directory.

## Install

```sh
git clone git@github.com:dreygur/cleaner.git
cd cleaner
make install          # builds and installs to ~/.local/bin/cleaner
```

Or grab a prebuilt binary from [Releases](https://github.com/dreygur/cleaner/releases).

## Usage

Scan and report, delete nothing:

```sh
cleaner --dry-run
```

Scan, then confirm each group interactively:

```sh
cleaner
```

Delete every matched group without prompting:

```sh
cleaner --yes
```

### Flags

| Flag | Meaning |
| --- | --- |
| `--dry-run` | Report what would be deleted, then exit |
| `--yes` | Delete every matched group without prompting |
| `--only <ids>` | Comma-separated rule ids to run exclusively |
| `--skip <ids>` | Comma-separated rule ids to leave alone |
| `--roots <dirs>` | Comma-separated directories to search for build artifacts |
| `--version` | Print version and exit |

Examples:

```sh
cleaner --only rust-target,node-modules --yes
cleaner --skip trash,chrome-cache
cleaner --roots ~/work,~/oss --dry-run
```

## Rules

Each rule targets one class of reclaimable data and carries its own regeneration
instructions, printed alongside the size in every report.

| Id | What it removes | How it comes back |
| --- | --- | --- |
| `rust-target` | Cargo build output | `cargo build` |
| `node-modules` | Installed npm dependencies | `npm install` / `pnpm install` |
| `next-build` | Next.js build output | `next build` |
| `pycache` | Compiled Python bytecode | Regenerated on next import |
| `go-build` | Go build cache | `go build` |
| `pnpm-store` | pnpm package store | `pnpm install` |
| `brew-cache` | Downloaded Homebrew bottles | Refetched on demand |
| `pip-cache` | pip wheel cache | Refetched on next install |
| `js-tool-cache` | node-gyp and TypeScript caches | Refetched on demand |
| `chrome-cache` | Chrome on-disk HTTP cache | Refills as you browse |
| `cargo-registry` | Downloaded crate sources | `cargo fetch` (needs network) |
| `xcode-derived` | Xcode DerivedData | Rebuilt by Xcode |
| `trash` | Trash | Nothing, this is a permanent delete |

Build artifacts are searched under `~/Code`, `~/Projects`, `~/src`, `~/work`,
and `~/dev` by default. Override with `--roots`.

## Safety

Deletion is permanent, nothing is moved to Trash first. The guards below are
enforced on every path, immediately before it is removed, and each one is
covered by a test:

- Paths outside `$HOME` are rejected.
- `$HOME` itself is rejected, as are `Documents`, `Desktop`, `Downloads`,
  `Pictures`, `Movies`, `Music`, `Library`, `Applications`, and `Public`.
- Symlinks are rejected, so a link can never be followed somewhere it should
  not reach.
- `target`, `node_modules`, and `.next` match only when the expected marker file
  (`Cargo.toml` or `package.json`) sits beside them. A directory named `target`
  in a notes folder is left alone.
- Cache directories are emptied but kept, so the owning tool still finds them.
  Build artifact directories are removed whole, since they are recreated as a
  unit.
- A failure on one path does not stop the remaining paths in its group.

Chrome's *profile* (history, passwords, extensions) and container or VM volumes
are deliberately outside every rule.

## Reported sizes

Sizes are the sum of apparent file sizes. On APFS, sparse files and transparent
compression mean the space actually reclaimed can be somewhat lower than the
figure in the report.

## Development

```sh
make test     # go test ./...
make vet      # go vet ./...
make build    # builds bin/cleaner
make scan     # builds, then runs --dry-run
```

## License

MIT
