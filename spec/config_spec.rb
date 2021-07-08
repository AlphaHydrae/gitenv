require 'helper'

describe Gitenv::Config, fakefs: true do
  subject{ Gitenv::Config.new }

  its(:repos){ should be_empty }
  its(:actions){ should be_empty }

  it "should raise an error when trying to symlink a file without a repo" do
    expect{ subject.symlink 'foo' }.to raise_error(RuntimeError, "You must specify a repository or a source directory to symlink from")
  end

  it "should raise an error when trying to copy a file without a repo" do
    expect{ subject.copy 'foo' }.to raise_error(RuntimeError, "You must specify a repository or a source directory to copy from")
  end

  context "with a repo" do
    before(:each){ subject.repo '/source' }
    its(:repos){ should eq([ Gitenv::Repository.new('/source') ]) }

    context "with a configuration" do
      before :each do
        subject.symlink 'foo'
        subject.copy 'bar'
        subject.copy 'baz'
      end

      its(:actions){ should have(3).items }

      it "should support including other files" do
        FileUtils.mkdir_p('/tmp')
        File.open('/tmp/config.rb', 'w') do |f|
          f.write %{
            symlink 'foo'
            copy 'corge'
          }
        end

        subject.include '/tmp/config.rb'
        expect(subject.actions).to have(5).items
      end

      it "should not support including relative paths" do
        expect{ subject.include 'foo' }.to raise_error(RuntimeError, "Only absolute paths can be included")
        expect{ subject.include './bar/baz' }.to raise_error(RuntimeError, "Only absolute paths can be included")
        expect{ subject.include '../qux' }.to raise_error(RuntimeError, "Only absolute paths can be included")
      end
    end
  end
end
