# encoding: UTF-8
require 'fileutils'
require 'digest/sha1'

module Gitenv

  class Copy

    class Action < Action

      def initialize context, files, options
        super context, Copy, files, options
      end

      def overwrite *args
        options = args.last.kind_of?(Hash) ? args.pop : {}
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

    def initialize context, file, options = {}
      @context, @file = context, file
      @as, @overwrite, @backup = options[:as], options[:overwrite], options[:backup]
      @mkdir = options.fetch :mkdir, true
      @backup = true if @overwrite && !options.key?(:backup)
    end

    def apply
      # TODO: test with mkdir option set to false
      FileUtils.mkdir_p File.dirname(target) if @mkdir
      backup_exists = File.exist? target_backup
      FileUtils.mv target, target_backup if @backup && File.exist?(target) && !backup_exists
      FileUtils.rm target if @overwrite && File.exist?(target) && !backup_exists
      FileUtils.cp origin, target unless File.exist?(target)
    end

    def to_s
      "#{Paint[target, :cyan]} #{Paint['<', :bold]} #{origin}"
    end

    def status
      if !File.exist?(target)
        Status.missing "is not set up; apply will create the copy"
      elsif @overwrite == false || digest(origin) == digest(target)
        Status.ok 'ok'
      elsif !@overwrite
        Status.error "already exists; enable overwrite if you want to replace it"
      elsif @backup && File.exist?(target_backup)
        Status.error "already exists with backup copy"
      else
        backup_notice = if @backup; " backup the file and"; end
        Status.warning "already exists; apply will#{backup_notice} overwrite"
      end
    end

    def origin
      @origin ||= File.join(*[ @context.from, @file ].compact)
    end

    def target
      @target ||= File.join(*[ @context.to, target_name].compact)
    end

    def target_backup
      @target_backup ||= "#{target}.orig"
    end

    private

    def target_name
      @target_name ||= @as || @file
    end

    def digest file
      Digest::SHA1.new.tap do |dig|
        File.open(file, 'rb'){ |io| dig.update io.readpartial(4096) while !io.eof }
      end
    end
  end
end
