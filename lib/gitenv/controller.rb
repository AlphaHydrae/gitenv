
module Gitenv

  class Controller

    def initialize action, options

      @action = action
      @options = options

      @home = File.expand_path '~'
      @config = Config.new @home
    end

    def run
      load_config_file!
      @config.actions.each &:build!
      @config.actions.each &:update! if @action == :update
      @config.actions.each{ |a| puts a }
    end

    private

    def load_config_file!
      
      config_file = @options.config ? File.expand_path(@options.config) : File.join(@home, '.gitenv.rb')
      if !File.exists?(config_file) and @options.config
        abort "No such configuration file #{config_file}"
      elsif !File.exists?(config_file)
        abort "You must create a configuration file #{config_file} or load another one with the --config option"
      elsif !File.file?(config_file)
        abort "Configuration file #{config_file} is not a file"
      elsif !File.readable?(config_file)
        abort "Configuration file #{config_file} is not readable"
      end

      contents = File.open(config_file, 'r').read
      @config.instance_eval contents, config_file
    end

    def abort msg, code = 1
      warn ms
      exit code
    end
  end
end
