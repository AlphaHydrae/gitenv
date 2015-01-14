# encoding: UTF-8
require 'paint'

module Gitenv
  VERSION = '0.2.0'
end

Dir[File.join File.dirname(__FILE__), File.basename(__FILE__, '.*'), '*.rb'].each{ |lib| require lib }
