# gitenv

Symlink manager for git repositories with configuration files.

    # This will create symlinks in your home folder
    # to all dot files in your environment repository.
    gitenv --repo ~/projects/env update

If you create a <a href="#configuration">configuration file</a>, you can drop the repo option:

    # Show current configuration.
    gitenv

    # Creates the symlinks.
    gitenv update

Read on for <a href="#configuration">more advanced options</a>.

## Installation

    gem install gitenv

<a name="configuration"></a>
## Configuration

If your repository is more complex than a bunch of dot files or you want to put the links somewhere other than in your home folder, you will have to create a configuration file. Gitenv looks for `~/.gitenv.rb` by default. You can override this with the `-c, --config` option or by setting the `GITENV_CONFIG` environment variable.

To create a basic configuration file, call gitenv with the `config` action.

    # Will prompt you for the path to your environment repository.
    gitenv config

    # Or you can provide it as an option.
    gitenv --repo ~/projects/env config

The following sections demonstrate the various options you can customize in the configuration file.

### Repository

```ruby
# Allows you to call gitenv without specifying the -r, --repo option.
repo '~/projects/env'

# Note that you can also set the GITENV\_REPO environment variable.
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

# Paths are relative to your home folder. If you need an absolute path, use #to\_abs.
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

## License (MIT)

Copyright (c) 2011 Alpha Hydrae

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
