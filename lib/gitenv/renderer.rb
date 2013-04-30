
module Gitenv

  class Renderer

    def initialize options = {}
      @options = options
    end

    def render action

      status = action.status
      color = status.color
      error = if !status.ok?
        "\n   #{Paint[status.message, color]}"
      end

      " #{Paint[status.marker, color]} #{action}#{error}"
    end
  end
end
