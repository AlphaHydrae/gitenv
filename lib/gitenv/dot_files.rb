
module Gitenv

  class DotFiles < AllFiles

    def files
      super.select{ |f| f.match /^\.[^\.]/ }
    end
  end
end
