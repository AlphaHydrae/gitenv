
module Gitenv

  module Context
    attr_accessor :from_paths, :to_paths, :absolute

    def from path, &block
      (@from_paths ||= []) << path
      if block
        instance_eval &block
        @from_paths.pop
      end
      self
    end

    def from_path
      @from_paths ? File.join(*([ @config.repository, @from_paths ].flatten)) : @config.repository
    end

    def to path, &block
      (@to_paths ||= []) << path
      if block
        instance_eval &block
        @to_paths.pop
      end
      self
    end

    def to_abs path, &block
      previous = @to_paths
      @to_paths = [ path ]
      @absolute = true
      if block
        instance_eval &block
        @to_paths = previous
        @absolute = false
      end
      self
    end

    def to_path
      @to_paths ? File.join(*(@absolute ? @to_paths : [ @config.home, @to_paths ]).flatten) : @config.home
    end

    def copy! config
      self.from_paths = config.from_paths ? config.from_paths.dup : []
      self.to_paths = config.to_paths ? config.to_paths.dup : []
      self.absolute = config.absolute
      @config = config
    end

    def home
      @config.home
    end

    def repository
      @config.repository
    end
  end
end
