# frozen_string_literal: true

require 'rubygems'
require 'bundler'

begin
  Bundler.setup(:default, :development)
rescue Bundler::BundlerError => e
  warn e.message
  warn 'Run `bundle install` to install missing gems'
  exit e.status_code
end

unless ENV['COVERAGE_DISABLED']
  require 'simplecov'
  require 'codecov'

  coverage_formatters = [
    SimpleCov::Formatter::HTMLFormatter,
    SimpleCov::Formatter::SimpleFormatter
  ]

  # Upload code coverage to https://codecov.io on continuous integration
  # environment.
  coverage_formatters.push(SimpleCov::Formatter::Codecov) if ENV['CI']

  SimpleCov.start do
    formatter SimpleCov::Formatter::MultiFormatter.new(coverage_formatters)
  end
end

RSpec.configure do |config|
  config.before :each, fakefs: true do
    FakeFS.activate!
    # necessary because fakefs doesn't implement readpartial
    allow_any_instance_of(Gitenv::Copy).to receive(:digest){ |_c, f| Digest::MD5.hexdigest File.read(f) }
  end

  config.after :each, fakefs: true do
    FakeFS.deactivate!
    FakeFS::FileSystem.clear
  end
end

require 'rspec'
require 'rspec/collection_matchers'
require 'rspec/its'

Dir["#{File.dirname(__FILE__)}/support/**/*.rb"].sort.each{ |f| require f }

require 'fakefs/spec_helpers'

require 'gitenv'
require 'ostruct'
