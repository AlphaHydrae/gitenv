require 'rubygems'
require 'bundler'

begin
  Bundler.setup(:default, :development)
rescue Bundler::BundlerError => e
  $stderr.puts e.message
  $stderr.puts "Run `bundle install` to install missing gems"
  exit e.status_code
end

require 'simplecov'
SimpleCov.start

RSpec.configure do |config|

  config.before :each, fakefs: true do
    FakeFS.activate!
    # necessary because fakefs doesn't implement readpartial
    allow_any_instance_of(Gitenv::Copy).to receive(:digest){ |c,f| Digest::MD5.hexdigest File.read(f) }
  end

  config.after :each, fakefs: true do
    FakeFS.deactivate!
    FakeFS::FileSystem.clear
  end
end

require 'rspec'
require 'rspec/its'

Dir[File.dirname(__FILE__) + "/support/**/*.rb"].each{ |f| require f }

require 'fakefs/spec_helpers'

require 'gitenv'
require 'ostruct'
