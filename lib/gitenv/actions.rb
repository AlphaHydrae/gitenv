module Gitenv
  class Action
    attr_reader :options

    def initialize(context, type, files, options)
      @context = context
      @type = type
      @files = files
      @options = options
    end

    def each(&block)
      @files.files(@context.from).each do |f|
        block.call @type.new(@context, f, @options)
      end
    end

    def each_file(&block)
      @files.files(@context.from).each do |f|
        block.call File.join(@context.from, f)
      end
    end

    %w[from to].each do |m|
      define_method m do |*args|
        @context.send(*(args.unshift m))
        self
      end
    end
  end
end

Dir[File.join File.dirname(__FILE__), File.basename(__FILE__, '.*'), '*.rb'].each { |lib| require lib }
