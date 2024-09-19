require 'rbconfig'

module Gitenv
  class Context
    attr_accessor :ignores

    def initialize(config, options = {})
      @config = config
      @from = options[:from]

      @to ||= File.expand_path('~')

      @ignores = options[:ignores] || []
      @ignores << '.DS_Store' if @ignores.empty? and RbConfig::CONFIG['host_os'] =~ /darwin/
    end

    def from(path = nil, &block)
      return @from if path.nil?

      old = @from
      @from = @from ? File.expand_path(path, @from) : File.expand_path(path)

      if block
        @config.instance_eval(&block)
        @from = old
      end

      self
    end

    def to(path = nil, &block)
      return @to if path.nil?

      old = @to
      @to = @to ? File.expand_path(path, @to) : File.expand_path(path)

      if block
        @config.instance_eval(&block)
        @to = old
      end

      self
    end

    def dup
      Context.new @config, from: @from, to: @to, ignores: @ignores.dup
    end
  end
end
