
module Gitenv

  class Action
    include Context
    
    def initialize config, type, files, options
      @type, @files, @options = type, files, options
      copy! config
    end

    def each &block
      @files.each from_path do |f|
        block.call @type.new(self, f, @options)
      end
    end

    def each_file &block
      @files.each from_path do |f|
        block.call File.join(from_path, f)
      end
    end
  end
end
