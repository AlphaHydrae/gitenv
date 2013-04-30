require 'helper'

describe Gitenv::Copy, fakefs: true do

  let(:file){ '.zshlogin' }
  let(:source){ '/source' }
  let(:source_file){ File.join source, file }
  let(:target){ File.expand_path '~' }
  let(:target_file){ File.join target, file }
  let(:context){ OpenStruct.new from: source, to: target }
  let(:options){ {} }
  subject{ Gitenv::Copy.new context, file, options }

  before :each do
    # TODO: fakefs pull request to support giving mkdir_p a list of directories
    FileUtils.mkdir_p source
    FileUtils.mkdir_p target
    File.open(source_file, 'w'){ |f| f.write 'foo' }
    @source_mtime = File.mtime source_file
  end

  its(:origin){ should eq(source_file) }
  its(:target){ should eq(target_file) }
  its(:target_backup){ should eq("#{target_file}.orig") }
  its(:to_s){ should eq_unpainted("#{target_file} < #{source_file}") }
  its(:status){ should be_status(:missing, /apply/) }

  context "when applied" do
    before(:each){ subject.apply }
    its(:origin){ should_not have_changed(@source_mtime) }
    its(:target){ should have_changed(@target_mtime) }
    its(:target){ should contain(File.read(source_file)) }
  end

  context "if the file is already copied" do

    before :each do
      File.open(target_file, 'w'){ |f| f.write File.read(source_file) }
      @target_mtime = File.mtime target_file
    end

    its(:status){ should be_status(:ok, /ok/) }

    context "when applied" do
      before(:each){ subject.apply }
      its(:origin){ should_not have_changed(@source_mtime) }
      its(:target){ should_not have_changed(@target_mtime) }
    end
  end

  context "if the file already exists with different contents" do

    before :each do
      File.open(target_file, 'w'){ |f| f.write 'bar' }
      @target_mtime = File.mtime target_file
    end

    its(:status){ should be_status(:error, /already exists/, /enable overwrite/) }

    context "when applied" do
      before(:each){ subject.apply }
      its(:origin){ should_not have_changed(@source_mtime) }
      its(:target){ should_not have_changed(@target_mtime) }
    end

    context "with the overwrite option set to false" do

      let(:options){ { overwrite: false } }
      its(:status){ should be_status(:ok, /ok/) }

      context "when applied" do
        before(:each){ subject.apply }
        its(:origin){ should_not have_changed(@source_mtime) }
        its(:target){ should_not have_changed(@target_mtime) }
      end
    end

    context "with the overwrite option" do

      let(:options){ { overwrite: true } }
      its(:status){ should be_status(:warning, /already exists/, /backup the file/, /overwrite/) }

      context "when applied" do
        before(:each){ subject.apply }
        its(:origin){ should_not have_changed(@source_mtime) }
        its(:target){ should have_changed(@target_mtime) }
        its(:target){ should contain(File.read(source_file)) }
        its(:target_backup){ should contain('bar') }
      end

      context "if the backup file already exists" do

        before :each do
          File.open("#{target_file}.orig", 'w'){ |f| f.write 'foo' }
        end

        its(:status){ should be_status(:error, /already exists/, /backup copy/) }

        context "when applied" do
          before(:each){ subject.apply }
          its(:origin){ should_not have_changed(@source_mtime) }
          its(:target){ should_not have_changed(@target_mtime) }
        end
      end

      context "with the backup option set to false" do

        let(:options){ { overwrite: true, backup: false } }
        its(:status){ should be_status(:warning, /already exists/, /apply will overwrite/) }

        context "when applied" do
          before(:each){ subject.apply }
          its(:origin){ should_not have_changed(@source_mtime) }
          its(:target){ should have_changed(@target_mtime) }
          its(:target){ should contain(File.read(source_file)) }
          its(:target_backup){ should_not exist }
        end
      end
    end
  end
end
