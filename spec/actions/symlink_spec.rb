require 'helper'

describe Gitenv::Symlink, :fakefs do
  subject { Gitenv::Symlink.new context, file, options }

  let(:file) { '.zshlogin' }
  let(:target) { '/source' }
  let(:target_file) { File.join target, file }
  let(:link_dir) { File.expand_path '~' }
  let(:link_mkdir) { true }
  let(:link) { File.join link_dir, file }
  let(:context) { OpenStruct.new from: target, to: link_dir }
  let(:options) { {} }

  before do
    FileUtils.mkdir_p target
    FileUtils.mkdir_p link_dir if link_mkdir
    File.open(target_file, 'w') { |f| f.write 'foo' }
    @target_mtime = File.mtime target_file
  end

  its(:link) { is_expected.to eq(link) }
  its(:target) { is_expected.to eq(target_file) }
  its(:to_s) { is_expected.to eq_unpainted("#{link} -> #{target_file}") }
  its(:status) { is_expected.to be_status(:missing, /apply/) }

  context 'when applied' do
    before { subject.apply }

    its(:target) { is_expected.not_to have_changed(@target_mtime) }
    its(:link) { is_expected.to link_to(target_file) }
  end

  context 'if the target directory does not exist' do
    let(:link_mkdir) { false }
    let(:link_dir) { '/missing/target/directory' }

    before do
      expect(File.exist?(link_dir)).to be(false)
      subject.apply
    end

    its(:target) { is_expected.not_to have_changed(@target_mtime) }
    its(:link) { is_expected.to link_to(target_file) }
  end

  context 'if the link already exists' do
    before do
      File.symlink target_file, link
    end

    its(:status) { is_expected.to be_status(:ok, /ok/) }

    context 'when applied' do
      before { subject.apply }

      its(:target) { is_expected.not_to have_changed(@target_mtime) }
      its(:link) { is_expected.to link_to(target_file) }
    end
  end

  context 'if the link points somewhere else' do
    let(:wrong_target_file) { File.join target, '.zshrc' }

    before do
      FileUtils.touch wrong_target_file
      File.symlink wrong_target_file, link
    end

    its(:status) { is_expected.to be_status(:error, /exists/, /enable overwrite/) }

    context 'when applied' do
      before { subject.apply }

      its(:target) { is_expected.not_to have_changed(@target_mtime) }
      its(:link) { is_expected.to link_to(wrong_target_file) }
    end

    context 'with the overwrite option set to false' do
      let(:options) { { overwrite: false } }

      its(:status) { is_expected.to be_status(:ok, /ok/) }

      context 'when applied' do
        before { subject.apply }

        its(:target) { is_expected.not_to have_changed(@target_mtime) }
        its(:link) { is_expected.to link_to(wrong_target_file) }
      end
    end

    context 'with the overwrite option' do
      let(:options) { { overwrite: true } }

      its(:status) { is_expected.to be_status(:warning, /currently points/, /apply will/) }

      context 'when applied' do
        before { subject.apply }

        its(:target) { is_expected.not_to have_changed(@target_mtime) }
        its(:link) { is_expected.to link_to(target_file) }
        # FIXME: fakefs seems to create a file instead of a symlink when moving a symlink
        # its(:link_backup){ should link_to(wrong_target_file) }
        its(:link_backup) { is_expected.to exist }
      end

      context 'if the backup file already exists' do
        before do
          File.open("#{link}.orig", 'w') { |f| f.write 'foo' }
        end

        its(:status) { is_expected.to be_status(:error, /already exists/, /backup copy/) }

        context 'when applied' do
          before { subject.apply }

          its(:target) { is_expected.not_to have_changed(@target_mtime) }
          its(:link) { is_expected.to link_to(wrong_target_file) }
        end
      end

      context 'with the backup option set to false' do
        let(:options) { { overwrite: true, backup: false } }

        its(:status) { is_expected.to be_status(:warning, /currently points/, /apply will overwrite/) }

        context 'when applied' do
          before { subject.apply }

          its(:target) { is_expected.not_to have_changed(@target_mtime) }
          its(:link) { is_expected.to link_to(target_file) }
          its(:link_backup) { is_expected.not_to exist }
        end
      end
    end
  end

  context 'if a file exists where the link would be' do
    before { File.open(link, 'w') { |f| f.write 'bar' } }

    its(:status) { is_expected.to be_status(:error, /exists/, /not a symlink/, /apply will ignore/) }

    context 'when applied' do
      before { subject.apply }

      its(:target) { is_expected.not_to have_changed(@target_mtime) }
      it('does not change the file') { expect(File.read(link)).to eq('bar') }
      it('does not delete the file') { expect(File.file?(link)).to be(true) }
    end

    context 'with the overwrite option' do
      let(:options) { { overwrite: true } }

      its(:status) { is_expected.to be_status(:warning, /exists/, /is not a symlink/, /apply will/) }

      context 'when applied' do
        before { subject.apply }

        its(:target) { is_expected.not_to have_changed(@target_mtime) }
        its(:link) { is_expected.to link_to(target_file) }
        its(:link_backup) { is_expected.to contain('bar') }
      end

      context 'if the backup file already exists' do
        before do
          File.open("#{link}.orig", 'w') { |f| f.write 'foo' }
        end

        its(:status) { is_expected.to be_status(:warning, /exists/, /not a symlink/, /backup/, /overwrite/) }

        context 'when applied' do
          before { subject.apply }

          its(:target) { is_expected.not_to have_changed(@target_mtime) }
          its(:link) { is_expected.to contain('bar') }
          its(:link_backup) { is_expected.to contain('foo') }
        end
      end

      context 'with the backup option set to false' do
        let(:options) { { overwrite: true, backup: false } }

        its(:status) { is_expected.to be_status(:warning, /exists/, /not a symlink/, /apply will overwrite/) }

        context 'when applied' do
          before { subject.apply }

          its(:target) { is_expected.not_to have_changed(@target_mtime) }
          its(:link) { is_expected.to link_to(target_file) }
          its(:link_backup) { is_expected.not_to exist }
        end
      end
    end
  end

  context 'if a directory exists where the link would be' do
    before { FileUtils.mkdir_p link }

    its(:status) { is_expected.to be_status(:error, /exists/, /not a symlink/, /apply will ignore/) }

    context 'when applied' do
      before { subject.apply }

      its(:target) { is_expected.not_to have_changed(@target_mtime) }
      it('does not delete the directory') { expect(File.directory?(link)).to be(true) }
    end
  end
end
