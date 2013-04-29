
module Gitenv

  class AllFiles < FileEnumerator

    def files path
      [ :filter, :ignore ].inject(Dir.entries(path)){ |memo,op| send op, path, memo }
    end

    private

    def filter path, entries
      entries.select{ |e| File.file? File.join(path, e) }
    end

    def ignore path, entries
      entries.reject{ |e| ignores.any?{ |g| g == e or (g.kind_of?(Regexp) and e.match(g)) } }
    end
  end
end
