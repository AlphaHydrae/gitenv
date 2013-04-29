
module Gitenv

  class Action
    include Context
    
    def initialize config, type, files, options
      @type, @files, @options = type, files, options
      copy! config
    end

    def each options = {}, &block
      @files.each from do |f|
        block.call @type.new(self, f, @options.merge(options))
      end
    end

    def each_file &block
      @files.each from do |f|
        block.call File.join(from, f)
      end
    end
  end
end
