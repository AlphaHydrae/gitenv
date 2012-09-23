
module Gitenv

  class FileEnumerator

    def files path
      raise '#files not implemented'
    end

    def each path, &block
      files(path).each{ |f| block.call f }
    end
  end
end
