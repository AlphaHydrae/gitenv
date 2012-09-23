# encoding: UTF-8
require 'paint'

module Gitenv
  VERSION = '0.0.4'
end

[ :context, :config, :controller, :symlink, :copy, :enumerator, :action ].each do |lib|
  require File.join(File.dirname(__FILE__), 'gitenv', lib.to_s)
end
