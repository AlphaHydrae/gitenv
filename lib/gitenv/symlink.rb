# encoding: UTF-8

module Gitenv

  class Symlink

    def initialize config, file
      @config, @file = config, file
    end

    def update!
      unless File.exists? link
        File.symlink target, link
      end
    end

    def to_s
      color, mark, msg = status
      %/ #{Paint[mark, color]} #{Paint[link, :cyan]} -> #{target}   #{Paint[msg, color]}/
    end

    private

    def status
      if File.symlink? link
        current_target = File.expand_path File.readlink(link)
        if current_target == target
          [ :green, "✓", "ok" ]
        else
          [ :yellow, "✗", "currently points to #{current_target}" ]
        end
      elsif File.file? link
        [ :red, "✗", "is a file" ]
      elsif File.directory? link
        [ :red, "✗", "is a directory" ]
      elsif File.exists? link
        [ :red, "✗", "exists but is not a symlink" ]
      else
        [ :blue, "✓", "is not set up" ]
      end
    end

    def link
      @link ||= File.join(*[ @config.to_path, @file].compact)
    end

    def target
      @target ||= File.join(*[ @config.from_path, @file ].compact)
    end
  end
end
