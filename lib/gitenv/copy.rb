
module Gitenv

  class Copy
    include Context

    def initialize config, file
      @config, @file = config, file
      self.from_paths = config.from_paths.dup
      self.to_paths = config.to_paths.dup
    end
  end
end
