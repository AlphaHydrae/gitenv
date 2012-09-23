
module Gitenv

  class AllFiles < FileEnumerator

    def files path
      Dir.entries(path).select{ |f| File.file? File.join(path, f) }
    end
  end
end
