require 'helper'

describe Gitenv::AllFiles, :fakefs do
  subject { matcher.files(source) }

  let(:options) { {} }
  let(:matcher) { Gitenv::AllFiles.new options }
  let(:source) { '/source' }
  let(:dirs) { ['dir', '.vim'] }
  let(:files) { ['file.txt', '.dotfile', 'README'] }

  before do
    dirs.each { |d| FileUtils.mkdir_p File.join(source, d) }
    files.each { |f| FileUtils.touch File.join(source, f) }
  end

  it { is_expected.to match_array(files) }

  context 'when given a filename to ignore' do
    let(:options) { { ignores: 'README' } }

    it { is_expected.to contain_exactly('file.txt', '.dotfile') }
  end

  context 'when given a pattern to ignore' do
    let(:options) { { ignores: /file/ } }

    it { is_expected.to contain_exactly('README') }
  end

  context 'when given multiple ignores' do
    let(:options) { { ignores: [/file/, 'README'] } }

    it { is_expected.to be_empty }
  end
end
