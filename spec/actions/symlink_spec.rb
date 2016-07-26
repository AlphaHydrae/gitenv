require 'helper'

describe Gitenv::Symlink, fakefs: true do

  let(:file){ '.zshlogin' }
  let(:target){ '/source' }
  let(:target_file){ File.join target, file }
  let(:link_dir){ File.expand_path '~' }
  let(:link){ File.join link_dir, file }
  let(:context){ OpenStruct.new from: target, to: link_dir }
  let(:options){ {} }
  subject{ Gitenv::Symlink.new context, file, options }

  before :each do
    FileUtils.mkdir_p target
    FileUtils.mkdir_p link_dir
    File.open(target_file, 'w'){ |f| f.write 'foo' }
    @target_mtime = File.mtime target_file
  end

  its(:link){ should eq(link) }
  its(:target){ should eq(target_file) }
  its(:to_s){ should eq_unpainted("#{link} -> #{target_file}") }
  its(:status){ should be_status(:missing, /apply/) }

  context "when applied" do
    before(:each){ subject.apply }
    its(:target){ should_not have_changed(@target_mtime) }
    its(:link){ should link_to(target_file) }
  end

  context "if the link already exists" do

    before :each do
      File.symlink target_file, link
    end

    its(:status){ should be_status(:ok, /ok/) }

    context "when applied" do
      before(:each){ subject.apply }
      its(:target){ should_not have_changed(@target_mtime) }
      its(:link){ should link_to(target_file) }
    end
  end

  context "if the link points somewhere else" do

    let(:wrong_target_file){ File.join target, '.zshrc' }
    before :each do
      FileUtils.touch wrong_target_file
      File.symlink wrong_target_file, link
    end

    its(:status){ should be_status(:error, /exists/, /enable overwrite/) }

    context "when applied" do
      before(:each){ subject.apply }
      its(:target){ should_not have_changed(@target_mtime) }
      its(:link){ should link_to(wrong_target_file) }
    end

    context "with the overwrite option set to false" do

      let(:options){ { overwrite: false } }
      its(:status){ should be_status(:ok, /ok/) }

      context "when applied" do
        before(:each){ subject.apply }
        its(:target){ should_not have_changed(@target_mtime) }
        its(:link){ should link_to(wrong_target_file) }
      end
    end

    context "with the overwrite option" do

      let(:options){ { overwrite: true } }
      its(:status){ should be_status(:warning, /currently points/, /apply will/) }

      context "when applied" do
        before(:each){ subject.apply }
        its(:target){ should_not have_changed(@target_mtime) }
        its(:link){ should link_to(target_file) }
        # FIXME: fakefs seems to create a file instead of a symlink when moving a symlink
        #its(:link_backup){ should link_to(wrong_target_file) }
        its(:link_backup){ should exist }
      end

      context "if the backup file already exists" do

        before :each do
          File.open("#{link}.orig", 'w'){ |f| f.write 'foo' }
        end

        its(:status){ should be_status(:error, /already exists/, /backup copy/) }

        context "when applied" do
          before(:each){ subject.apply }
          its(:target){ should_not have_changed(@target_mtime) }
          its(:link){ should link_to(wrong_target_file) }
        end
      end

      context "with the backup option set to false" do

        let(:options){ { overwrite: true, backup: false } }
        its(:status){ should be_status(:warning, /currently points/, /apply will overwrite/) }

        context "when applied" do
          before(:each){ subject.apply }
          its(:target){ should_not have_changed(@target_mtime) }
          its(:link){ should link_to(target_file) }
          its(:link_backup){ should_not exist }
        end
      end
    end
  end

  context "if a file exists where the link would be" do
    before(:each){ File.open(link, 'w'){ |f| f.write 'bar' } }
    its(:status){ should be_status(:error, /exists/, /not a symlink/, /apply will ignore/) }

    context "when applied" do
      before(:each){ subject.apply }
      its(:target){ should_not have_changed(@target_mtime) }
      it("should not change the file"){ expect(File.read(link)).to eq('bar') }
      it("should not delete the file"){ expect(File.file? link).to be(true) }
    end

    context "with the overwrite option" do

      let(:options){ { overwrite: true } }
      its(:status){ should be_status(:warning, /exists/, /is not a symlink/, /apply will/) }

      context "when applied" do
        before(:each){ subject.apply }
        its(:target){ should_not have_changed(@target_mtime) }
        its(:link){ should link_to(target_file) }
        its(:link_backup){ should contain('bar') }
      end

      context "if the backup file already exists" do

        before :each do
          File.open("#{link}.orig", 'w'){ |f| f.write 'foo' }
        end

        its(:status){ should be_status(:warning, /exists/, /not a symlink/, /backup/, /overwrite/) }

        context "when applied" do
          before(:each){ subject.apply }
          its(:target){ should_not have_changed(@target_mtime) }
          its(:link){ should contain('bar') }
          its(:link_backup){ should contain('foo') }
        end
      end

      context "with the backup option set to false" do

        let(:options){ { overwrite: true, backup: false } }
        its(:status){ should be_status(:warning, /exists/, /not a symlink/, /apply will overwrite/) }

        context "when applied" do
          before(:each){ subject.apply }
          its(:target){ should_not have_changed(@target_mtime) }
          its(:link){ should link_to(target_file) }
          its(:link_backup){ should_not exist }
        end
      end
    end
  end

  context "if a directory exists where the link would be" do
    before(:each){ FileUtils.mkdir_p link }
    its(:status){ should be_status(:error, /exists/, /not a symlink/, /apply will ignore/) }

    context "when applied" do
      before(:each){ subject.apply }
      its(:target){ should_not have_changed(@target_mtime) }
      it("should not delete the directory"){ expect(File.directory? link).to be(true) }
    end
  end
end
