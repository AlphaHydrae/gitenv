module Gitenv
  class Repository
    attr_reader :path

    def initialize(path)
      @path = path
    end

    def ==(other)
      other.path == path
    end
  end
end
