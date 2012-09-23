
module Gitenv

  module Context

    def from path, &block
      (@from_path ||= []) << path
      if block
        instance_eval &block
        @from_path.pop
      end
    end

    def from_path
      @from_path ? File.join(*@from_path) : nil
    end

    def from_paths
      @from_path || []
    end

    def from_paths= paths
      @from_path = paths
    end

    def to path, &block
      (@to_path ||= []) << path
      if block
        instance_eval &block
        @to_path.pop
      end
    end

    def to_path
      @to_path ? File.join(*@to_path) : nil
    end

    def to_paths
      @to_path || []
    end

    def to_paths= paths
      @to_path = paths
    end
  end
end
