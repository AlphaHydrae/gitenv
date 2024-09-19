module Gitenv
  class Status
    TYPES = %i[ok missing warning error]
    COLORS = { ok: :green, missing: :blue, warning: :yellow, error: :red }

    attr_reader :type, :message

    class << self
      TYPES.each do |m|
        define_method m do |message|
          new m, message
        end
      end
    end

    TYPES.each do |m|
      define_method "#{m}?" do
        @type == m
      end
    end

    def marker
      @type == :ok ? '✓' : '✗'
    end

    def color
      COLORS[@type]
    end

    private

    def initialize(type, message)
      @type = type
      @message = message
    end
  end
end
