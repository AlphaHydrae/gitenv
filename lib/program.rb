require File.join File.dirname(__FILE__), 'gitenv'
require 'commander/import'

program :name, 'gitenv'
program :version, Gitenv::VERSION
program :description, 'Symlink manager for git repositories with configuration files.'

global_option '-r', '--repo PATH', 'Specify the path to the environment repository'
global_option '-c', '--config PATH', 'Use a custom configuration file (defaults to ~/.gitenv.rb)'

command :info do |c|
  c.syntax = 'gitenv info'
  c.description = 'Display the current configuration (default action)'
  c.action do |_args, options|
    Gitenv::Controller.new(:info, options).run
  end
end

command :apply do |c|
  c.syntax = 'gitenv apply'
  c.description = 'Create/update the symlinks'
  c.action do |_args, options|
    Gitenv::Controller.new(:apply, options).run
  end
end

# TODO: add a verbose option (show repo, show config file path)
# TODO: add link to documentation in help

default_command :info
