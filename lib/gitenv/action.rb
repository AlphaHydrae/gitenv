
module Gitenv

  class Action
    include Context
    
    def initialize config, type, files
      @type, @files = type, files
      copy! config
    end

    def each &block
      @files.each File.join(*[repository, from_path].compact) do |f|
        block.call @type.new(self, f)
      end
    end
  end
end
