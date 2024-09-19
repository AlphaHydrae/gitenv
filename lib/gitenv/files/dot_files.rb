module Gitenv
  class DotFiles < AllFiles
    def files(path)
      super(path).select { |f| f.match(/^\.[^.]/) }
    end
  end
end
