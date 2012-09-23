# encoding: UTF-8
require 'paint'

module Gitenv
  VERSION = '0.0.3'
end

[ :context, :config, :controller, :symlink, :copy, :enumerator, :all_files, :dot_files, :actions ].each do |lib|
  require File.join(File.dirname(__FILE__), 'gitenv', lib.to_s)
end
