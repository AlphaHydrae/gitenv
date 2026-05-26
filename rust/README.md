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

```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: "."
    configs:
      - .zshrc
```

`repository` points to the root of the environment repository on disk.

### Defaults

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

### Rename targets

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

### Sub-folders in the repository

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

### Change the destination

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

### Select multiple files

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

### Overwrite and backup

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

### Composition with includes

Use `includes` to reference other config files for composition:

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
