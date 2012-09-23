# encoding: UTF-8

module Gitenv

  class Symlink
    include Context

    def initialize config, file
      @config, @file = config, file
      copy! config
    end

    def build!
      @color, @mark, @message = if File.symlink? link
        @current_target = File.expand_path File.readlink(link)
        if @current_target == target
          [ :green, "✓", "ok" ]
        else
          [ :yellow, "✗", "currently points to #{@current_target}" ]
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

    def update!
      if !File.exists? link
        File.symlink target, link
        @color, @mark, @message = :green, "✓", "ok"
      end
    end

    def to_s
      %/ #{status_mark} #{Paint[link, :cyan]} -> #{target}   #{status_message}/
    end

    private

    def status_mark
      Paint["#{@mark}", @color]
    end

    def status_message
      Paint["#{@message}", @color]
    end

    def link
      File.join(*[ @config.home, to_path, @file].compact)
    end

    def target
      File.join(*[ @config.repository, from_path, @file ].compact)
    end
  end
end
