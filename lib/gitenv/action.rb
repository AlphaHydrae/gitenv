
module Gitenv

  class Action
    include Context
    
    def initialize config, type, files
      @type, @files = type, files
      copy! config
    end

    def each &block
      @files.each from_path do |f|
        block.call @type.new(self, f)
      end
    end

    def each_file &block
      @files.each from_path do |f|
        block.call File.join(from_path, f)
      end
    end
  end
end
