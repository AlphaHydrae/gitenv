
module Gitenv

  module Context
    attr_accessor :home, :repository
    attr_accessor :from_paths, :to_paths

    def from path, &block
      (@from_paths ||= []) << path
      if block
        instance_eval &block
        @from_paths.pop
      end
    end

    def from_path
      @from_paths ? File.join(*@from_paths) : nil
    end

    def to path, &block
      (@to_paths ||= []) << path
      if block
        instance_eval &block
        @to_paths.pop
      end
    end

    def to_path
      @to_paths ? File.join(*@to_paths) : nil
    end

    def copy! source
      self.from_paths = source.from_paths ? source.from_paths.dup : []
      self.to_paths = source.to_paths ? source.to_paths.dup : []
      self.repository = source.repository
      self.home = source.home
    end
  end
end
