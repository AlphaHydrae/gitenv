require 'helper'

describe Gitenv::FilesMatcher, fakefs: true do

  let(:options){ {} }
  let(:matcher){ Gitenv::FilesMatcher.new options }
  let(:source){ '/source' }
  let(:dirs){ [ 'dir', '.vim' ] }
  let(:files){ [ 'file.txt', '.dotfile', 'README' ] }
  let(:extra){ [ '.', '..' ] }

  before :each do
    dirs.each{ |d| FileUtils.mkdir_p File.join(source, d) }
    files.each{ |f| FileUtils.touch File.join(source, f) }
  end

  subject{ matcher.files(source) }
  it{ should match_array(dirs + files + extra) }

  context "with an exception" do
    subject{ matcher.except('dir', 'README').files(source) }
    it{ should match_array(dirs + files + extra - [ 'dir', 'README' ]) }
  end
end
