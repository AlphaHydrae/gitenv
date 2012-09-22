
module Gitenv

  class Config
    attr_reader :actions
    attr_reader :repository
    attr_reader :home

    def initialize home
      @home = home
      @actions = []
    end

    def repo path
      @repository = File.expand_path path
    end

    def symlink file, *args # TODO: remove args
      Symlink.new(self, file).tap do |action|
        @actions << action
      end
    end

    def configure &block
      @self_before_instance_eval = eval "self", block.binding
      instance_eval &block
      @self_before_instance_eval = nil
    end

    def method_missing method, *args, &block
      @self_before_instance_eval.send method, *args, &block
    end
  end
end
