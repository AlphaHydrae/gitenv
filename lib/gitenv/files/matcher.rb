module Gitenv
  class FilesMatcher
    def initialize(options = {})
      @options = options
      @ignores = options[:ignores] ? [options[:ignores]].flatten : []
    end

    def files(path)
      ignore Dir.entries(path)
    end

    def except *args
      @ignores.concat args
      self
    end

    private

    def ignore(entries)
      entries.reject { |e| @ignores.any? { |g| g == e or (g.is_a?(Regexp) and e.match(g)) } }
    end
  end
end
