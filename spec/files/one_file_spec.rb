require 'helper'

describe Gitenv::OneFile, fakefs: true do

  let(:matcher){ Gitenv::OneFile.new file }
  let(:source){ '/source' }
  let(:files){ [ 'file.txt', '.dotfile', 'README' ] }
  let(:file){ 'README' }

  before :each do
    FileUtils.mkdir_p source
    files.each{ |f| FileUtils.touch File.join(source, f) }
  end

  subject{ matcher.files(source) }
  it{ should match_array([ file ]) }
end
