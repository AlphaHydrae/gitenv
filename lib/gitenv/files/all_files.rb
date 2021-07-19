# frozen_string_literal: true

module Gitenv
  class AllFiles < FilesMatcher
    def files(path)
      filter path, super(path)
    end

    private

    def filter(path, entries)
      entries.select{ |e| File.file? File.join(path, e) }
    end
  end
end
