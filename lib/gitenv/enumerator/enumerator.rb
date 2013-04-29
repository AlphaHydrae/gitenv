
module Gitenv

  class FileEnumerator

    def initialize options = {}
      @options = options
    end

    def files path
      []
    end

    def each path, &block
      files(path).each &block
    end

    private

    def ignores
      @ignores ||= @options[:ignores] ? [ @options[:ignores] ].flatten : []
    end
  end
end
