# encoding: UTF-8
require 'fileutils'
require 'digest/sha1'

module Gitenv

  class Copy

    def initialize config, file, options = {}
      @config, @file, @options = config, file, options
    end

    def update!
      if File.exists?(target) && !File.exists?(target_copy)
        FileUtils.mv target, target_copy
      end
      if !File.exists?(target)
        FileUtils.copy source, target
      end
    end

    def to_s
      color, mark, msg = status
      %/ #{Paint[mark, color]} #{Paint[target, :cyan]} << #{source}   #{Paint[msg, color]}/
    end

    private

    def status
      if !File.exists?(target)
        [ :blue, "✗", "is not yet set up" ]
      elsif digest(source) == digest(target)
        [ :green, "✓", "ok" ]
      elsif File.exists?(target_copy)
        [ :red, "✗", "already exists with backup copy" ]
      else
        [ :red, "✗", "already exists" ]
      end
    end

    def target
      @target ||= File.join(*[ @config.to_path, target_name].compact)
    end

    def target_copy
      @target_copy ||= "#{target}.orig"
    end

    def target_name
      @options[:as] || @file
    end

    def source
      @source ||= File.join(*[ @config.from_path, @file ].compact)
    end

    def digest file
      Digest::SHA1.new.tap do |dig|
        File.open(file, 'rb'){ |io| dig.update io.readpartial(4096) while !io.eof }
      end
    end
  end
end
