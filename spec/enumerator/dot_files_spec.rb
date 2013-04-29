require 'helper'

describe Gitenv::DotFiles do
  include FakeFS::SpecHelpers

  subject{ Gitenv::DotFiles.new options }
  let(:files){ [ 'file.txt', '.zshrc', 'README', '.vimrc', '.gitignore' ] }
  let(:dot_files){ [ '.zshrc', '.vimrc', '.gitignore' ] }
  let(:options){ {} }

  before :each do
    @tmp = Dir.mktmpdir
    files.each{ |f| FileUtils.touch File.join(@tmp, f) }
  end

  it "should list dot files" do
    subject.files(@tmp).should match_array(dot_files)
  end

  it "should enumerate dot files" do
    result = []
    subject.each(@tmp){ |f| result << f }
    result.should match_array(dot_files)
  end

  it "should not list directories" do
    Dir.mkdir File.join(@tmp, 'dir')
    subject.files(@tmp).should match_array(dot_files)
  end

  context "when given a filename to ignore" do

    let(:options){ { :ignores => '.vimrc' } }

    it "should ignore that file" do
      subject.files(@tmp).should match_array([ '.zshrc', '.gitignore' ])
    end
  end

  context "when given a pattern to ignore" do

    let(:options){ { :ignores => /rc/ } }

    it "should ignore matching files" do
      subject.files(@tmp).should match_array([ '.gitignore' ])
    end
  end

  context "when given multiple ignores" do

    let(:options){ { :ignores => [ /rc/, '.gitignore' ] } }

    it "should ignore matching files" do
      subject.files(@tmp).should be_empty
    end
  end
end
