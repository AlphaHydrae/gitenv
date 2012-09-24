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
  c.action do |args,options|
    Gitenv::Controller.new(:info, options).run
  end
end

command :update do |c|
  c.syntax = 'gitenv update'
  c.description = 'Create/update the symlinks'
  c.action do |args,options|
    Gitenv::Controller.new(:update, options).run
  end
end

command :config do |c|
  c.syntax = 'gitenv config'
  c.description = 'Create a basic configuration file'
  c.action do |args,options|
    Gitenv::Controller.new(:config, options).run
  end
end

# TODO: add a :format action to describe the configuration file DSL
# TODO: add a :config action to create a basic configuration file
# TODO: add a verbose option (show repo, show config file path)

default_command :info

