require 'helper'

describe Gitenv::OneFile do
  include FakeFS::SpecHelpers

  subject{ Gitenv::OneFile.new file }
  let(:files){ [ 'file.txt', '.dotfile', 'README' ] }
  let(:file){ 'README' }

  before :each do
    @tmp = Dir.mktmpdir
    files.each{ |f| FileUtils.touch File.join(@tmp, f) }
  end

  it "should list the given file" do
    subject.files(@tmp).should match_array([ file ])
  end

  it "should enumerate the given file" do
    result = []
    subject.each(@tmp){ |f| result << f }
    result.should match_array([ file ])
  end
end
