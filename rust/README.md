# gitenv Rust Port (Temporary README)

This README is specific to the Rust port while migration is in progress.
Do not treat it as the final root documentation yet.

Run `gitenv` without arguments to inspect the planned links and copies.

```text
~/.zshrc -> ~/projects/dotfiles/.zshrc   not yet set up
~/.gitconfig <- ~/projects/dotfiles/.gitconfig   not yet set up
```

Then run `gitenv apply` to create the missing targets.

```text
created symlink ~/.zshrc -> ~/projects/dotfiles/.zshrc
copied ~/projects/dotfiles/.gitconfig to ~/projects/dotfiles/.gitconfig
```

## Installation

Installation instructions are not finalized yet.

Placeholder:

```text
<installation instructions will be added here>
```

## Configuration

gitenv reads YAML configuration from one of the following locations:

1. `$GITENV_CONFIG` (if set)
2. `$XDG_CONFIG_HOME/gitenv/config.yml` (if set)
3. `$HOME/.config/gitenv/config.yml` (fallback)

Start with a minimal configuration:

<!-- readme-config-id: minimal -->

```yaml
version: 1
repository: "~/projects/env"

defaults:
  mkdir: true

sources:
  - from: "."
    configs:
      - .zshrc
```

The sections below explain the main parts of that file and the common options.

### Repository

<!-- readme-config-id: repository -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: "."
    configs:
      - .zshrc
```

`repository` points to the root of the environment repository on disk.

At runtime, `repository` can be overridden without editing YAML:

1. `--repo PATH`
2. `GITENV_REPO`

Examples:

```sh
gitenv --repo ~/projects/other-env
GITENV_REPO=~/projects/other-env gitenv
```

### Defaults

<!-- readme-config-id: defaults -->

```yaml
version: 1
repository: "~/projects/env"

defaults:
  mode: symlink
  to: "~"
  mkdir: true
  overwrite: false
  backup_on_overwrite: false

sources:
  - from: "."
    configs:
      - .zshrc
```

`defaults` sets the shared behavior for all config items unless a source or an
individual item overrides it.

### Change the destination

<!-- readme-config-id: destination -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: "."
    to: ".config/gitenv-demo"
    configs:
      - .zshrc
      - .gitconfig
```

`to` is resolved relative to the home directory when it is not absolute.

### Copy files

<!-- readme-config-id: copy-files -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: "."
    configs:
      - file: .gitconfig
        mode: copy
      - file: .config/tool.json
        to: ".config/tool"
        mode: copy
```

Set `mode: copy` when a target should be copied instead of symlinked.

### Sub-folders in the repository

<!-- readme-config-id: sub-folders -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: zsh
    configs:
      - .zshrc
  - from: editor
    configs:
      - settings.json
      - keybindings.json
```

Each `from` value is a source root inside the repository. This is the YAML
equivalent of grouping entries under a repository sub-directory.

### Rename targets

<!-- readme-config-id: rename-targets -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: "."
    configs:
      - zshrc.local
      - file: zshrc.shared
        as: .zshrc.local
```

Use `as` with the `file:` form to give the target a different name from the
source file. This is useful when the installed name must differ from the name
in the repository. The shorthand `- filename` form uses the same name for
source and target.

### Overwrite and backup

<!-- readme-config-id: overwrite-and-backup -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: "."
    configs:
      - file: .zshrc
        overwrite: true
      - file: .gitconfig
        mode: copy
        overwrite: true
        backup_on_overwrite: true
```

`overwrite: true` replaces existing targets. `backup_on_overwrite: true` keeps
the previous target as a sibling `.orig` path before replacement.

### File-level mkdir override

<!-- readme-config-id: file-item-mkdir -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: "."
    configs:
      - file: .config/tool/config.toml
        to: .config/tool
        mkdir: true
      - file: .zshrc
```

Use `mkdir` on an individual `file` item to override directory-creation
behavior without changing defaults for all other items.

### Environment-backed source roots

<!-- readme-config-id: environment-backed-source-roots -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: $DOTFILES_ROOT
    configs:
      - .zshrc
  - from:
      env: TOOL_CONFIG_ROOT
      optional: true
    to: .config/tool
    configs:
      - config.toml
```

`from: $VAR` is shorthand for an environment-backed source. Use the canonical
object form when you need `optional: true`.

### Source-level guards

<!-- readme-config-id: source-level-guards -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: "."
    to: .config/gitenv-demo
    when: to_exists
    configs:
      - .zshrc
  - from: macos
    when:
      directory_exists: /Applications
    configs:
      - karabiner.json
```

Use `when` to include a source only when a condition is satisfied. `to_exists`
checks the resolved destination directory for that source, and
`directory_exists` checks an explicit path.

### Select multiple files

<!-- readme-config-id: select-multiple-files -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: profiles
    to: ".local/share/profiles"
    configs:
      - select:
          type: dot
          include:
            - "dotfiles/**"
          exclude:
            - "**/*.tmp"
            - "private/**"
```

`select` expands a set of files from one source directory. Set `type` to:

- `dot` for names that start with `.`
- `non-dot` for names that do not start with `.`
- `all` for both dot and non-dot names

`include` uses glob patterns matched against source-relative paths. When it is
present, a file must match at least one include pattern before the excludes
are applied. This keeps recursive selectors predictable when you want to carve
out a smaller subset from a larger tree.

`exclude` uses glob patterns matched against source-relative paths. For example:

- `"**/*.tmp"` excludes temporary files anywhere in recursive selection
- `"private/**"` excludes everything under `private/` from the source root
- `"**/.DS_Store"` excludes Finder metadata files in any directory

Selection precedence is deterministic: `type` selects candidates first, then
`include` narrows the candidate set when present, then `exclude` removes any
remaining matches.

On macOS, `.DS_Store` is excluded automatically during selector expansion.
Add it to `exclude` if you want to make the rule explicit for a source.

### Recursive select for existing target directories

<!-- readme-config-id: recursive-select-existing-directories-only -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: profiles
    to: .local/share/profiles
    configs:
      - select:
          type: all
          recursive: true
          existing_directories_only: true
          include:
            - "**/*.profile"
```

Use `existing_directories_only: true` with `recursive: true` to avoid creating
new target directory chains during recursive selection. The selector still
plans entries, but apply only touches targets whose parent directory chain
already exists.

### Select item-level overrides

<!-- readme-config-id: select-item-overrides -->

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: profiles
    configs:
      - select:
          type: non-dot
          include:
            - "**/*.json"
          mode: copy
          to: .config/profiles
          mkdir: true
          overwrite: true
          backup_on_overwrite: true
```

Use per-select overrides when one selector needs different behavior from
defaults or source-level options.

### Composition with includes

Use `includes` to reference other config files for composition:

<!-- readme-config-id: composition-with-includes -->

```yaml
version: 1
repository: "~/projects/env"

includes:
  - shared-config.yml
  - ~/.gitenv-private.yml

defaults:
  mkdir: true

sources:
  - from: "."
    configs:
      - .zshrc
```

Relative include paths are resolved relative to the file that declares them.
Absolute paths and paths starting with `~` are used as-is. Environment
variable references like `$GITENV_PRIVATE` resolve to the path stored in that
variable. The `includes` list is processed in order; an including file's own
sources always appear before the included file's sources.

### Optional includes

<!-- readme-config-id: optional-includes -->

```yaml
version: 1
repository: "~/projects/env"

includes:
  - path: shared-config.yml
    optional: false
  - path: ~/.local/gitenv-private.yml
    optional: true
  - env: CUSTOM_GITENV_CONFIG
    optional: true

sources:
  - from: "."
    configs:
      - .zshrc
```

Use `optional: true` on `path` and `env` includes to silently skip missing files
or environment variables. When `optional` is not specified, includes are required.

### Command-line flags

gitenv can be configured at runtime with the following flags and environment variables that override YAML values.

#### `--config PATH` / `-c PATH`

Override the config file path for the current command.

```sh
gitenv --config ~/.gitenv-custom.yml info
gitenv -c ~/.gitenv-custom.yml apply
GITENV_CONFIG=~/.gitenv-custom.yml gitenv info
```

#### `--repo PATH`

Override the repository root path for the current command.

```sh
gitenv --repo ~/projects/other-env info
GITENV_REPO=~/projects/other-env gitenv apply
```

#### `--log-level LEVEL`

Set log output level: `debug`, `info`, `warn` (default), `error`.

```sh
gitenv --log-level debug info
gitenv --log-level error apply
```

#### `--color MODE`

Control color output mode: `auto` (default), `yes`, `no`.

```sh
gitenv --color=no info
gitenv --color=yes apply
GITENV_COLOR=yes gitenv info
```
