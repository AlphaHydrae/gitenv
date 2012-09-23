
module Gitenv

  class Config
    include Context

    attr_reader :actions
    attr_reader :repository
    attr_reader :home

    def initialize home
      @home = home
      @actions = Actions.new
    end

    def repo path
      @repository = File.expand_path path
    end

    def symlink file

      scope = self
      current_actions = Actions.new
      enumerator(file).each do |f|
        current_actions << Symlink.new(scope, f).tap do |action|
          @actions << action
        end
      end

      current_actions
    end

    def all_files
      :all_files
    end

    def dot_files
      :dot_files
    end

    private

    def enumerator file
      if file == :all_files
        AllFiles.new File.join(*[@repository, from_path].compact)
      elsif file == :dot_files
        DotFiles.new File.join(*[@repository, from_path].compact)
      else
        [ file ]
      end
    end
  end
end
