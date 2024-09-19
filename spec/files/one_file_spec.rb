require 'helper'

describe Gitenv::OneFile, :fakefs do
  subject { matcher.files(source) }

  let(:matcher) { Gitenv::OneFile.new file }
  let(:source) { '/source' }
  let(:files) { ['file.txt', '.dotfile', 'README'] }
  let(:file) { 'README' }

  before do
    FileUtils.mkdir_p source
    files.each { |f| FileUtils.touch File.join(source, f) }
  end

  it { is_expected.to contain_exactly(file) }
end
