require 'helper'

describe Gitenv::AllFiles, fakefs: true do

  let(:options){ {} }
  let(:matcher){ Gitenv::AllFiles.new options }
  let(:source){ '/source' }
  let(:dirs){ [ 'dir', '.vim' ] }
  let(:files){ [ 'file.txt', '.dotfile', 'README' ] }

  before :each do
    dirs.each{ |d| FileUtils.mkdir_p File.join(source, d) }
    files.each{ |f| FileUtils.touch File.join(source, f) }
  end

  subject{ matcher.files(source) }
  it{ should match_array(files) }

  context "when given a filename to ignore" do

    let(:options){ { :ignores => 'README' } }
    it{ should match_array([ 'file.txt', '.dotfile' ]) }
  end

  context "when given a pattern to ignore" do

    let(:options){ { :ignores => /file/ } }
    it{ should match_array([ 'README' ]) }
  end

  context "when given multiple ignores" do

    let(:options){ { :ignores => [ /file/, 'README' ] } }
    it{ should be_empty }
  end
end
