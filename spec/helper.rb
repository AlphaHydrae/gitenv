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
  require 'simplecov-cobertura'

  coverage_formatters = [
    SimpleCov::Formatter::CoberturaFormatter,
    SimpleCov::Formatter::HTMLFormatter,
    SimpleCov::Formatter::SimpleFormatter
  ]

  SimpleCov.start do
    formatter SimpleCov::Formatter::MultiFormatter.new(coverage_formatters)
  end
end

RSpec.configure do |config|
  config.before :each, :fakefs do
    FakeFS.activate!
    # necessary because fakefs doesn't implement readpartial
    allow_any_instance_of(Gitenv::Copy).to receive(:digest) { |_c, f| Digest::MD5.hexdigest File.read(f) }
  end

  config.after :each, :fakefs do
    FakeFS.deactivate!
    FakeFS::FileSystem.clear
  end
end

require 'rspec'
require 'rspec/collection_matchers'
require 'rspec/its'

Dir[File.dirname(__FILE__) + '/support/**/*.rb'].each { |f| require f }

require 'fakefs/spec_helpers'

require 'gitenv'
require 'ostruct'
