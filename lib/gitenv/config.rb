require 'rbconfig'

module Gitenv

  class Config

    attr_reader :repos
    attr_reader :actions

    def initialize
      @context = Context.new self
      @repos, @actions = [], []
    end

    def repo path, &block
      @repos << Repository.new(path)
      @context.from path, &block
      self
    end

    def symlink file, options = {}
      raise "You must specify a repository or a source directory to symlink from" unless @context.from
      Symlink::Action.new(@context.dup, matcher(file), options).tap{ |a| @actions << a }
    end

    def copy file, options = {}
      raise "You must specify a repository or a source directory to copy from" unless @context.from
      Copy::Action.new(@context.dup, matcher(file), options).tap{ |a| @actions << a }
    end

    def all_files options = {}
      matcher :all_files, options
    end

    def dot_files options = {}
      matcher :dot_files, options
    end

    %w(from to).each do |m|
      define_method m do |*args,&block|
        @context.send *(args.unshift m), &block
        self
      end
    end

    def ignores
      @context.ignores
    end

    def include other_config_file, optional: false
      absolute_path = File.expand_path(other_config_file)
      unless File.exists?(absolute_path)
        raise "Cannot find file to include #{absolute_path}" unless optional
        return
      end

      contents = File.open(file, 'r').read
      self.instance_eval contents, file
    end

    private

    def matcher file, options = {}
      options[:ignores] ||= @context.ignores.dup
      if file.kind_of? FilesMatcher
        file
      elsif file == :all_files
        AllFiles.new options
      elsif file == :dot_files
        DotFiles.new options
      else
        OneFile.new file, options
      end
    end
  end
end
