require 'helper'

describe Gitenv::AllFiles do
  include FakeFS::SpecHelpers

  subject{ Gitenv::AllFiles.new options }
  let(:files){ [ 'file.txt', '.dotfile', 'README' ] }
  let(:options){ {} }

  before :each do
    @tmp = Dir.mktmpdir
    files.each{ |f| FileUtils.touch File.join(@tmp, f) }
  end

  it "should list all files" do
    subject.files(@tmp).should match_array(files)
  end

  it "should enumerate all files" do
    result = []
    subject.each(@tmp){ |f| result << f }
    result.should match_array(files)
  end

  it "should not list directories" do
    Dir.mkdir File.join(@tmp, 'dir')
    subject.files(@tmp).should match_array(files)
  end

  context "when given a filename to ignore" do

    let(:options){ { :ignores => 'README' } }

    it "should ignore that file" do
      subject.files(@tmp).should match_array([ 'file.txt', '.dotfile' ])
    end
  end

  context "when given a pattern to ignore" do

    let(:options){ { :ignores => /file/ } }

    it "should ignore matching files" do
      subject.files(@tmp).should match_array([ 'README' ])
    end
  end

  context "when given multiple ignores" do

    let(:options){ { :ignores => [ /file/, 'README' ] } }

    it "should ignore matching files" do
      subject.files(@tmp).should be_empty
    end
  end
end
