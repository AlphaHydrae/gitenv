
module Gitenv

  class Actions

    def initialize
      @actions = []
    end

    def each &block
      @actions.each &block
    end

    def from path
      @actions.each{ |a| a.from path }
      @actions
    end

    def to path
      @actions.each{ |a| a.to path }
      @actions
    end

    def << action
      @actions << action
    end
  end
end
