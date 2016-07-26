# encoding: UTF-8

module Gitenv

  class Symlink

    class Action < Action

      def initialize context, files, options
        super context, Symlink, files, options
      end

      def overwrite overwrite = true
        @options[:overwrite] = overwrite
        self
      end

      def once
        @options[:overwrite] = false
        self
      end
    end

    def initialize context, file, options = {}
      @context, @file = context, file
      @as, @overwrite = options[:as], options[:overwrite]
      @mkdir = options.fetch :mkdir, true
    end

    def apply
      FileUtils.mkdir_p File.dirname(target) if @mkdir
      FileUtils.rm link if @overwrite && File.exists?(link) # FIXME: only if link points somewhere else
      File.symlink target, link unless File.symlink?(link) or File.exists?(link)
    end

    def to_s
      "#{Paint[link, :cyan]} #{Paint['->', :bold]} #{target}"
    end

    def status
      if File.symlink? link
        current_target = File.expand_path File.readlink(link)
        if @overwrite == false or current_target == target
          Status.ok 'ok'
        elsif !@overwrite
          Status.error "exists but points to #{current_target}; enable overwrite if you want to replace it"
        else
          Status.warning "currently points to #{current_target}; apply will overwrite"
        end
      elsif File.exists? link
        if @overwrite
          Status.warning "exists but is not a symlink; apply will overwrite"
        else
          Status.error "exists but is not a symlink; apply will ignore"
        end
      else
        Status.missing "is not set up; apply will create the link"
      end
    end

    def link
      @link ||= File.join(*[ @context.to, link_name].compact)
    end

    def target
      @target ||= File.join(*[ @context.from, @file ].compact)
    end

    private

    def link_name
      @link_name ||= @as || @file
    end
  end
end
