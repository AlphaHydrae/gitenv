module Gitenv
  class Symlink
    class Action < Action
      def initialize(context, files, options)
        super context, Symlink, files, options
      end

      def overwrite *args
        options = args.last.is_a?(Hash) ? args.pop : {}
        overwrite = args.empty? ? true : args.shift
        @options[:overwrite] = overwrite
        @options[:backup] = options[:backup] if options.key?(:backup)
        self
      end

      def once
        @options[:overwrite] = false
        @options[:backup] = false
        self
      end
    end

    def initialize(context, file, options = {})
      @context = context
      @file = file
      @as = options[:as]
      @overwrite = options[:overwrite]
      @backup = options[:backup]
      @mkdir = options.fetch :mkdir, true
      @backup = true if @overwrite && !options.key?(:backup)
    end

    def apply
      FileUtils.mkdir_p File.dirname(link) if @mkdir
      backup_exists = File.exist? link_backup
      FileUtils.mv link, link_backup if @backup && file_or_symlink_exists?(link) && !backup_exists
      return if File.symlink?(link) && File.readlink(link) == target

      # TODO: only if link points somewhere else
      FileUtils.rm link if @overwrite && file_or_symlink_exists?(link) && !backup_exists
      File.symlink target, link unless File.exist?(link)
    end

    def to_s
      "#{Paint[link, :cyan]} #{Paint['->', :bold]} #{target}"
    end

    def status
      backup_notice = (' backup the file and' if @backup)
      if File.symlink? link
        current_target = File.expand_path File.readlink(link)
        if @overwrite == false || current_target == target
          Status.ok 'ok'
        elsif !@overwrite
          Status.error "exists but points to #{current_target}; enable overwrite if you want to replace it"
        elsif @backup && File.exist?(link_backup)
          Status.error 'already exists with backup copy'
        else
          Status.warning "currently points to #{current_target}; apply will#{backup_notice} overwrite"
        end
      elsif File.exist? link
        if @overwrite
          Status.warning "exists but is not a symlink; apply will#{backup_notice} overwrite"
        else
          Status.error 'exists but is not a symlink; apply will ignore'
        end
      else
        Status.missing 'is not set up; apply will create the link'
      end
    end

    def link
      @link ||= File.join(*[@context.to, link_name].compact)
    end

    def link_backup
      @link_backup ||= "#{link}.orig"
    end

    def target
      @target ||= File.join(*[@context.from, @file].compact)
    end

    private

    def link_name
      @link_name ||= @as || @file
    end

    def file_or_symlink_exists?(path)
      File.symlink?(path) || File.exist?(path)
    end
  end
end
