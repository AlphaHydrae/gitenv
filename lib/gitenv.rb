# encoding: UTF-8
require 'paint'

module Gitenv
  VERSION = '0.0.2'

  class Runner

    def initialize
      @home = File.expand_path '~'
      @config_file = File.join @home, '.gitenv.rb'
    end

    def run

      @dsl = DSL.new
      @dsl.instance_eval File.open(@config_file, 'r').read, @config_file

      action = ARGV.shift || 'info'

      puts
      puts "gitenv v#{VERSION}"
      puts "configuration #{@config_file}"
      puts
      case action
      when 'info'
        puts Paint["Listing current state", :bright, :bold]
      when 'update'
        puts Paint["Creating missing links", :blue, :bold]
      end

      @dsl.symlinks.each do |link|

        link_file_path = File.join @home, link[:file]
        real_file = link[:source] || link[:file]
        real_file_path = File.join File.expand_path(@dsl.repository), real_file

        msg = %/#{Paint[link_file_path, :cyan]} -> #{real_file_path}/
        data = analyze link_file_path, real_file_path

        case action
        when 'info'
          puts %/#{info_mark data}#{msg}   #{info_msg data}/
        when 'update'
          unless File.exists? link_file_path
            File.symlink real_file_path, link_file_path
          end
          puts %/#{update_mark data}#{msg}   #{update_msg data}/
        else
          raise "No such action #{action}"
        end
      end

      puts
    end

    private

    def update_mark data
      case data[:status]
      when :setup
        Paint[" ✓ ", :green]
      when :wrong_link
        Paint[" ✗ ", :yellow]
      when :not_link
        Paint[" ✗ ", :yellow]
      when :void
        Paint[" ✓ ", :green]
      else
        Paint[" ? ", :red]
      end
    end

    def update_msg data
      case data[:status]
      when :setup
        Paint["already here", :green]
      when :wrong_link
        Paint["skipped (points to #{data[:target]})", :yellow]
      when :not_link
        Paint["skipped (not a symlink)", :yellow]
      when :void
        Paint["set up", :green]
      else
        Paint["unknown", :red]
      end
    end

    def info_msg data
      case data[:status]
      when :setup
        Paint["already here", :green]
      when :wrong_link
        Paint["points to #{data[:target]}", :yellow]
      when :not_link
        Paint["is not a symlink", :red]
      when :void
        Paint["is not yet set up", :blue]
      else
        Paint["unknown", :red]
      end
    end

    def info_mark data
      case data[:status]
      when :setup
        Paint[" ✓ ", :green]
      when :wrong_link
        Paint[" ✗ ", :red]
      when :not_link
        Paint[" ✗ ", :red]
      when :void
        Paint[" ✓ ", :green]
      else
        Paint[" ? ", :red]
      end
    end

    def analyze link_file_path, real_file_path
      if File.symlink? link_file_path
        current_real_file_path = File.expand_path(File.readlink(link_file_path))
        if current_real_file_path == real_file_path
          { :status => :setup }
        else
          { :status => :wrong_link, :target => current_real_file_path }
        end
      elsif File.exists? link_file_path
        { :status => :not_link }
      else
        { :status => :void }
      end
    end
  end

  class DSL
    attr_reader :repository
    attr_reader :symlinks

    def initialize
      @symlinks = []
    end

    def repo path
      @repository = File.expand_path path
      raise unless File.directory? @repository
    end

    def within path
      path = File.expand_path path
      raise unless File.directory? path
      @within = path
      yield
      @within = nil
    end

    def symlink file, source = nil
      source ||= file
      s = source || file
      f = File.join @repository, s
      raise "Unknown file #{f}" unless File.exists? f
      @symlinks << { :file => file, :source => source, :within => @within }
    end

    def configure &block
      @self_before_instance_eval = eval "self", block.binding
      instance_eval &block
      @self_before_instance_eval = nil
    end

    def method_missing method, *args, &block
      @self_before_instance_eval.send method, *args, &block
    end
  end
end
