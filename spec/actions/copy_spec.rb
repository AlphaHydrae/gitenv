require 'helper'

describe Gitenv::Copy, :fakefs do
  subject { Gitenv::Copy.new context, file, options }

  let(:file) { '.zshlogin' }
  let(:source) { '/source' }
  let(:source_file) { File.join source, file }
  let(:target) { File.expand_path '~' }
  let(:target_file) { File.join target, file }
  let(:target_mkdir) { true }
  let(:context) { OpenStruct.new from: source, to: target }
  let(:options) { {} }

  before do
    FileUtils.mkdir_p source
    FileUtils.mkdir_p target if target_mkdir
    File.open(source_file, 'w') { |f| f.write 'foo' }
    @source_mtime = File.mtime source_file
  end

  its(:origin) { is_expected.to eq(source_file) }
  its(:target) { is_expected.to eq(target_file) }
  its(:target_backup) { is_expected.to eq("#{target_file}.orig") }
  its(:to_s) { is_expected.to eq_unpainted("#{target_file} < #{source_file}") }
  its(:status) { is_expected.to be_status(:missing, /apply/) }

  context 'when applied' do
    before { subject.apply }

    its(:origin) { is_expected.not_to have_changed(@source_mtime) }
    its(:target) { is_expected.to have_changed(@target_mtime) }
    its(:target) { is_expected.to contain(File.read(source_file)) }
  end

  context 'if the target directory does not exist' do
    let(:target_mkdir) { false }
    let(:target) { '/missing/target/directory' }

    before do
      expect(File.exist?(target)).to be(false)
      subject.apply
    end

    its(:origin) { is_expected.not_to have_changed(@source_mtime) }
    its(:target) { is_expected.to have_changed(@target_mtime) }
    its(:target) { is_expected.to contain(File.read(source_file)) }
  end

  context 'if the file is already copied' do
    before do
      File.open(target_file, 'w') { |f| f.write File.read(source_file) }
      @target_mtime = File.mtime target_file
    end

    its(:status) { is_expected.to be_status(:ok, /ok/) }

    context 'when applied' do
      before { subject.apply }

      its(:origin) { is_expected.not_to have_changed(@source_mtime) }
      its(:target) { is_expected.not_to have_changed(@target_mtime) }
    end
  end

  context 'if the file already exists with different contents' do
    before do
      File.open(target_file, 'w') { |f| f.write 'bar' }
      @target_mtime = File.mtime target_file
    end

    its(:status) { is_expected.to be_status(:error, /already exists/, /enable overwrite/) }

    context 'when applied' do
      before { subject.apply }

      its(:origin) { is_expected.not_to have_changed(@source_mtime) }
      its(:target) { is_expected.not_to have_changed(@target_mtime) }
    end

    context 'with the overwrite option set to false' do
      let(:options) { { overwrite: false } }

      its(:status) { is_expected.to be_status(:ok, /ok/) }

      context 'when applied' do
        before { subject.apply }

        its(:origin) { is_expected.not_to have_changed(@source_mtime) }
        its(:target) { is_expected.not_to have_changed(@target_mtime) }
      end
    end

    context 'with the overwrite option' do
      let(:options) { { overwrite: true } }

      its(:status) { is_expected.to be_status(:warning, /already exists/, /backup the file/, /overwrite/) }

      context 'when applied' do
        before { subject.apply }

        its(:origin) { is_expected.not_to have_changed(@source_mtime) }
        its(:target) { is_expected.to have_changed(@target_mtime) }
        its(:target) { is_expected.to contain(File.read(source_file)) }
        its(:target_backup) { is_expected.to contain('bar') }
      end

      context 'if the backup file already exists' do
        before do
          File.open("#{target_file}.orig", 'w') { |f| f.write 'foo' }
        end

        its(:status) { is_expected.to be_status(:error, /already exists/, /backup copy/) }

        context 'when applied' do
          before { subject.apply }

          its(:origin) { is_expected.not_to have_changed(@source_mtime) }
          its(:target) { is_expected.not_to have_changed(@target_mtime) }
        end
      end

      context 'with the backup option set to false' do
        let(:options) { { overwrite: true, backup: false } }

        its(:status) { is_expected.to be_status(:warning, /already exists/, /apply will overwrite/) }

        context 'when applied' do
          before { subject.apply }

          its(:origin) { is_expected.not_to have_changed(@source_mtime) }
          its(:target) { is_expected.to have_changed(@target_mtime) }
          its(:target) { is_expected.to contain(File.read(source_file)) }
          its(:target_backup) { is_expected.not_to exist }
        end
      end
    end
  end
end
