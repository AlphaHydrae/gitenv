module Gitenv
  class Renderer
    def initialize(options = {})
      @options = options
    end

    def render(action)
      status = action.status
      color = status.color
      error = ("\n   #{Paint[status.message, color]}" unless status.ok?)

      " #{Paint[status.marker, color]} #{action}#{error}"
    end
  end
end
