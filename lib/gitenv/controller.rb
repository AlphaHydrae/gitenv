require 'readline'

module Gitenv
  class Controller
    def initialize(action, options)
      @action = action
      @options = options

      @config = Config.new
    end

    def run
      check_config_file!

      create_config_file! if !File.exist?(config_file) and !repository

      repo = repository
      @config.from repo if repo && !repo.empty?

      load_config_file!

      # @config.from repository
      # check_repository!

      # load dot files by default
      # if @config.actions.empty?
      #  @config.symlink @config.dot_files
      # end

      # check_files!

      renderer = Renderer.new
      @config.actions.each do |a|
        a.each do |impl|
          impl.apply if @action == :apply
          puts renderer.render(impl)
        end
      end
    end

    private

    def create_config_file!
      file = config_file
      unless agree "You have no configuration file (#{file}); do you wish to create one? (y/n) "
        puts
        abort "To use gitenv, you must either create a configuration file\nor specify an environment repository with the -r, --repo option."
      end

      repo = @options.repo || ENV['GITENV_REPO']
      unless repo
        Readline.completion_append_character = '/'
        Readline.completion_proc = proc do |str|
          Dir[str + '*'].grep(/^#{Regexp.escape(str)}/)
        end
        begin
          repo = Readline.readline('Type the path to your environment repository: ', true)
        rescue Interrupt
          exit 1
        end
      end

      if !repo or repo.strip.empty?
        puts
        abort 'You must specify an environment repository.'
      elsif !File.directory?(File.expand_path(repo))
        puts
        abort "No such directory #{repo}."
      end

      config = String.new.tap do |s|
        s << %(\n# Path to your environment repository.)
        s << %(\nrepo "#{repo}"\n)
        s << %(\n# Create symlinks in your home folder.)
        s << %(\nsymlink dot_files\n\n)
      end

      File.open(file, 'w') { |f| f.write config }

      puts
      puts Paint["Successfully wrote configuration to #{file}", :green]
      puts
    end

    def repository
      @options.repo || ENV['GITENV_REPO']
    end

    def config_file
      if @options.config
        File.expand_path @options.config
      elsif ENV['GITENV_CONFIG']
        File.expand_path ENV['GITENV_CONFIG']
      else
        File.expand_path '~/.gitenv.rb'
      end
    end

    def load_config_file!
      file = config_file
      return unless File.exist? file

      contents = File.open(file, 'r').read
      @config.instance_eval contents, file
    end

    def check_files!
      problems = []
      @config.actions.each do |a|
        a.each_file do |f|
          if !File.exist?(f)
            problems << { file: f, msg: 'does not exist' }
          # elsif !File.file?(f)
          #  problems << { :file => f, :msg => "is not a file" }
          elsif !File.readable?(f)
            problems << { file: f, msg: 'is not readable' }
          end
        end
      end

      return unless problems.any?

      msg = 'There are problems with the following files in your repository:'
      problems.each do |p|
        msg << "\n   #{p[:file]} #{p[:msg]}"
      end
      abort msg
    end

    def check_config_file!
      file = config_file
      return unless File.exist?(file)

      if !File.file?(file)
        abort "#{file} is not a file. It cannot be used as a configuration file."
      elsif !File.readable?(file)
        abort "#{file} is not readable. It cannot be used as a configuration file."
      end
    end

    def check_repository!
      unless @config.from
        msg = 'You have not specified an environment repository.'
        msg << "\nYou must either use the -r, --repo option or create"
        msg << "\na configuration file (~/.gitenv.rb by default) with"
        msg << "\nthe repo setting."
        abort msg
      end
      return if File.directory? @config.from

      notice = File.exist?(@config.from) ? 'is not a directory' : 'does not exist'
      from = if @options.repo
               "--repo #{@options.repo}"
             elsif ENV['GITENV_REPO']
               "$GITENV_REPO = #{ENV['GITENV_REPO']}"
             else
               %(repo "#{@config.from}")
             end
      abort "The repository you have specified #{notice}.\n   (#{from})"
    end

    def abort(msg, code = 1)
      warn Paint[msg, :red]
      exit code
    end
  end
end
