# frozen_string_literal: true

require 'helper'

describe Gitenv::DotFiles, fakefs: true do
  subject{ matcher.files(source) }

  let(:matcher){ described_class.new options }
  let(:source){ '/source' }
  let(:dirs){ ['dir', '.vim'] }
  let(:files){ ['file.txt', '.zshrc', 'README', '.vimrc', '.gitignore'] }
  let(:dot_files){ ['.zshrc', '.vimrc', '.gitignore'] }
  let(:options){ {} }

  before do
    dirs.each{ |d| FileUtils.mkdir_p File.join(source, d) }
    files.each{ |f| FileUtils.touch File.join(source, f) }
  end

  it{ is_expected.to match_array(dot_files) }

  context 'when given a filename to ignore' do
    let(:options){ { ignores: '.vimrc' } }

    it{ is_expected.to match_array(['.zshrc', '.gitignore']) }
  end

  context 'when given a pattern to ignore' do
    let(:options){ { ignores: /rc/ } }

    it{ is_expected.to match_array(['.gitignore']) }
  end

  context 'when given multiple ignores' do
    let(:options){ { ignores: [/rc/, '.gitignore'] } }

    it{ is_expected.to be_empty }
  end
end
