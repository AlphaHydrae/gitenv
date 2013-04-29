require 'rbconfig'

module Gitenv

  class Config
    include Context

    attr_reader :repos
    attr_reader :actions
    attr_reader :home

    def initialize home
      @home = home
      @repos = []
      @actions = []
      @ignores = []
      @ignores << '.DS_Store' if RbConfig::CONFIG['host_os'] =~ /darwin/
    end

    def repo path, &block
      @repos << Repository.new(path)
      from path, &block
    end

    def symlink file, options = {}
      raise "You must specify a repository or a source directory to symlink from" unless @from
      Action.new(self, Symlink, enumerator(file), options).tap{ |a| @actions << a }
    end

    def copy file, options = {}
      raise "You must specify a repository or a source directory to copy from" unless @from
      Action.new(self, Copy, enumerator(file), options).tap{ |a| @actions << a }
    end

    def all_files options = {}
      enumerator :all_files, options
    end

    def dot_files options = {}
      enumerator :dot_files, options
    end

    private

    def enumerator file, options = {}
      options[:ignores] ||= ignores
      if file.kind_of? FileEnumerator
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
