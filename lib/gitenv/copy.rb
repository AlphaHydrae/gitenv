
module Gitenv

  class Copy
    include Context

    def initialize config, file
      @config, @file = config, file
      copy! config
    end
  end
end
