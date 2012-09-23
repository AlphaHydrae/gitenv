
module Gitenv

  class FileEnumerator

    def initialize path
      @path = path
    end

    def files
      raise '#files not implemented'
    end

    def each &block
      files.each do |f|
        block.call f
      end
    end
  end
end
