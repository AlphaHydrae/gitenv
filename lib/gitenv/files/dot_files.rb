# frozen_string_literal: true

module Gitenv
  class DotFiles < AllFiles
    def files(path)
      super(path).select{ |f| f.match(/^\.[^.]/) }
    end
  end
end
