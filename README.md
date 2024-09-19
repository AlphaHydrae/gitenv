# gitenv

Creates symlinks to your configuration files in a git repository (<a href="https://github.com/AlphaHydrae/env">like mine</a>).

[![Gem Version](https://badge.fury.io/rb/gitenv.svg)](http://badge.fury.io/rb/gitenv)
[![Build](https://github.com/AlphaHydrae/gitenv/actions/workflows/build.yml/badge.svg)](https://github.com/AlphaHydrae/gitenv/actions/workflows/build.yml)
[![Code Coverage](https://codecov.io/gh/AlphaHydrae/gitenv/branch/main/graph/badge.svg?token=PHYTXEW2MG)](https://codecov.io/gh/AlphaHydrae/gitenv)

Run gitenv without arguments to check the symlink configuration. First-time users will be prompted to enter the path to their environment repository so gitenv can set up its own config file.

    #=> gitenv

     ~/.gemrc -> ~/projects/env/.gemrc           ok
     ~/.gitconfig -> ~/projects/env/.gitconfig   not yet set up
     ~/.zshrc -> ~/projects/env/.zshrc           not yet set up

Then run it with **apply** to set up the missing symlinks.

    #=> gitenv apply

     ~/.gemrc -> ~/projects/env/.gemrc           ok
     ~/.gitconfig -> ~/projects/env/.gitconfig   ok
     ~/.zshrc -> ~/projects/env/.zshrc           ok

Read on for <a href="#configuration">more advanced options</a>.

## Installation

    gem install gitenv

<a name="configuration"></a>
## Configuration

If your repository is more complex than a bunch of dot files or you want to put the links somewhere other than in your home folder, you will have to customize your configuration file. Gitenv prompts you to create one the first time you run it. It looks for `~/.gitenv.rb` by default. You can override this with the `-c, --config` option or by setting the `GITENV_CONFIG` environment variable.

The following sections demonstrate the various options you can customize in the configuration file.

### Repository

```ruby
# Path to your environment repository.
repo '~/projects/env'

# Note that you can also set the GITENV_REPO environment variable.
```

### Symlinks

```ruby
# This will create symlinks in your home folder to all dot files (e.g. .vimrc, .gitconfig) in the repository.
symlink dot_files

# This creates symlinks for all files, not just dot files.
symlink all_files

# You can also create symlinks one at a time.
symlink '.zshrc'
symlink 'my_config_file'

# If you want the name of link to be different than that of the file, use the :as option.
symlink 'zshconfig', :as => '.zshconfig'
```

### Sub-folders in the repository

```ruby
# If your configuration file or files are in a sub-folder, use #from to tell gitenv where to find them.
from 'subfolder' do
  symlink dot_files
end

# The following syntax is also available.
symlink('.zshrc').from('zsh')
symlink(all_files).from('a/b/c')

# You can chain several #from calls to traverse hierarchies.
from 'a' do
  from 'b' do
    from 'c' do
      # Symlink all dot files in a/b/c in your repository.
      symlink dot_files
    end
  end
end
```

### Change the destination

```ruby
# If you want to put the symlinks somewhere other than in your home folder, use #to.
to 'links' do
  symlink dot_files
end

# The following syntax is also available.
symlink('.zshrc').to('links')
symlink(all_files).to('a/b/c')

# Paths are relative to your home folder. If you need an absolute path, use #to_abs.
symlink('my_config_file').to_abs('/opt/config')
```

### Copy files

```ruby
# Gitenv can also copy files instead of creating symlinks.
copy dot_files

# Path modifiers work the same as for symlinks.
copy(dot_files).from('zsh')
copy('my_config_file').to('configs')
```

### Symlinks or copies in system directories

Just use `sudo`. Gitenv will happily set up your symlinks. You might have to add the `-E` switch to keep gitenv in the PATH.
