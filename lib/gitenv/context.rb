
module Gitenv

  module Context
    attr_accessor :ignores

    def from path = nil, &block
      return @from if path.nil?
      old = @from
      @from = @from ? File.expand_path(path, @from) : File.expand_path(path)
      if block
        instance_eval &block
        @from = old
      end
      self
    end

    def to path = nil, &block
      return @to || home if path.nil?
      old = @to
      @to = @to ? File.expand_path(path, @to) : File.expand_path(path)
      if block
        instance_eval &block
        @to = old
      end
      self
    end

    def copy! context
      @from = context.from
      @to = context.to
      @ignores = context.ignores
    end

    private

    def home
      @@home ||= File.expand_path('~')
    end
  end
end
