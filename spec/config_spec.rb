require 'helper'

describe Gitenv::Config, :fakefs do
  subject { Gitenv::Config.new }

  its(:repos) { is_expected.to be_empty }
  its(:actions) { is_expected.to be_empty }

  it 'raises an error when trying to symlink a file without a repo' do
    expect do
      subject.symlink 'foo'
    end.to raise_error(RuntimeError,
                       'You must specify a repository or a source directory to symlink from')
  end

  it 'raises an error when trying to copy a file without a repo' do
    expect do
      subject.copy 'foo'
    end.to raise_error(RuntimeError, 'You must specify a repository or a source directory to copy from')
  end

  context 'with a repo' do
    before { subject.repo '/source' }

    its(:repos) { is_expected.to eq([Gitenv::Repository.new('/source')]) }

    context 'with a configuration' do
      before do
        subject.symlink 'foo'
        subject.copy 'bar'
        subject.copy 'baz'
      end

      its(:actions) { is_expected.to have(3).items }

      it 'supports including other files' do
        FileUtils.mkdir_p('/tmp')
        File.open('/tmp/config.rb', 'w') do |f|
          f.write %(
            symlink 'foo'
            copy 'corge'
          )
        end

        subject.include '/tmp/config.rb'
        expect(subject.actions).to have(5).items
      end

      it 'does not support including relative paths' do
        expect { subject.include 'foo' }.to raise_error(RuntimeError, 'Only absolute paths can be included')
        expect { subject.include './bar/baz' }.to raise_error(RuntimeError, 'Only absolute paths can be included')
        expect { subject.include '../qux' }.to raise_error(RuntimeError, 'Only absolute paths can be included')
      end
    end
  end
end
