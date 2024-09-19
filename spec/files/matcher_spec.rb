require 'helper'

describe Gitenv::FilesMatcher, :fakefs do
  subject { matcher.files(source) }

  let(:options) { {} }
  let(:matcher) { Gitenv::FilesMatcher.new options }
  let(:source) { '/source' }
  let(:dirs) { ['dir', '.vim'] }
  let(:files) { ['file.txt', '.dotfile', 'README'] }
  let(:extra) { ['.', '..'] }

  before do
    dirs.each { |d| FileUtils.mkdir_p File.join(source, d) }
    files.each { |f| FileUtils.touch File.join(source, f) }
  end

  it { is_expected.to match_array(dirs + files + extra) }

  context 'with an exception' do
    subject { matcher.except('dir', 'README').files(source) }

    it { is_expected.to match_array(dirs + files + extra - %w[dir README]) }
  end
end
