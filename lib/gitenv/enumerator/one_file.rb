
module Gitenv

  class OneFile < FileEnumerator

    def initialize file
      @file = file
    end

    def files path
      [ @file ]
    end
  end
end
