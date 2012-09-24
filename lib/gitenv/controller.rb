require 'readline'

module Gitenv

  class Controller

    def initialize action, options

      @action = action
      @options = options

      @home = File.expand_path '~'
      @config = Config.new @home
    end

    def run

      return create_config_file if @action == :config

      check_config_file!
      load_config_file!

      @config.repository = ENV['GITENV_REPO'] if ENV['GITENV_REPO']
      @config.repository = @options.repo if @options.repo
      check_repository!

      # load dot files by default
      if @config.actions.empty?
        @config.symlink @config.dot_files
      end

      check_files!

      @config.actions.each do |a|
        a.each do |impl|
          impl.update! if @action == :update
          puts impl
        end
      end
    end

    private

    def create_config_file
      file = config_file
      if File.exists? file
        return puts "You already have a config file (#{file})."
      end
      repo = @options.repo || ENV['GITENV_REPO']
      unless repo
        Readline.completion_append_character = '/'
        repo = Readline.readline('Type the path to your environment repository: ', true)
      end
      repo_exists = repo && !repo.strip.empty? && File.directory?(File.expand_path(repo))
      repo = repo_exists ? File.expand_path(repo) : nil
      config = String.new.tap do |s|
        s << "\n# Path to your environment repository."
        if repo_exists
          s << "\nrepo '#{repo}'"
        else
          s << "\n# repo ''"
        end
        s << "\n"
        s << "\n# Create symlinks in the home folder."
        s << "\nsymlink dot_files"
        s << "\n"
      end
      File.open(file, 'w'){ |f| f.write config }
    end

    def config_file
      if @options.config
        File.expand_path @options.config
      elsif ENV['GITENV_CONFIG']
        File.expand_path ENV['GITENV_CONFIG']
      else
        File.join @home, '.gitenv.rb'
      end
    end

    def load_config_file!
      file = config_file
      return unless File.exists? file
      contents = File.open(file, 'r').read
      @config.instance_eval contents, file
    end

    def check_files!
      problems = []
      @config.actions.each do |a|
        a.each_file do |f|
          if !File.exists?(f)
            problems << { :file => f, :msg => "does not exist" }
          elsif !File.file?(f)
            problems << { :file => f, :msg => "is not a file" }
          elsif !File.readable?(f)
            problems << { :file => f, :msg => "is not readable" }
          end
        end
      end

      return unless problems.any?

      msg = "There are problems with the following files in your repository:"
      problems.each do |p|
        msg << "\n   #{p[:file]} #{p[:msg]}"
      end
      abort msg
    end

    def check_config_file!
      file = config_file
      return if !File.exists?(file) and !@options.config and !ENV['GITENV_CONFIG']
      if !File.exists?(file) and @options.config
        abort "No such configuration file.\n   (--config #{@options.config})"
      elsif !File.exists?(file)
        abort "No such configuration file.\n   ($GITENV_CONFIG = #{ENV['GITENV_CONFIG']})"
      elsif !File.file?(file)
        abort "#{file} is not a file."
      elsif !File.readable?(file)
        abort "#{file} is not readable."
      end
    end

    def check_repository!
      if !@config.repository
        msg = "You have not specified an environment repository."
        msg << "\nYou must either use the -r, --repo option or create"
        msg << "\na configuration file (~/.gitenv.rb by default) with"
        msg << "\nthe repo setting."
        abort msg
      end
      return if File.directory? @config.repository
      notice = File.exists?(@config.repository) ? 'is not a directory' : 'does not exist'
      from = if @options.repo
        "--repo #{@options.repo}"
      elsif ENV['GITENV_REPO']
        "$GITENV_REPO = #{ENV['GITENV_REPO']}"
      else
        %/repo "#{@config.repository}"/
      end
      abort "The repository you have specified #{notice}.\n   (#{from})"
    end

    def abort msg, code = 1
      warn Paint[msg, :red]
      exit code
    end
  end
end
