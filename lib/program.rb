require File.join File.dirname(__FILE__), 'gitenv'
require 'commander/import'

program :name, 'gitenv'
program :version, Gitenv::VERSION
program :description, 'Symlink manager for git repositories with configuration files.'

command :info do |c|
  c.action do |args,options|
    Gitenv::Controller.new(:info, options).run
  end
end

command :update do |c|
  c.action do |args,options|
    Gitenv::Controller.new(:update, options).run
  end
end

global_option '-c', '--config FILE', 'Use a custom configuration file (defaults to ~/.gitenv.rb)'

default_command :info

