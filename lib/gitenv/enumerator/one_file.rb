
module Gitenv

  class OneFile < FileEnumerator

    def initialize file, options = {}
      super options
      @file = file
    end

    def files path
      [ @file ]
    end
  end
end
