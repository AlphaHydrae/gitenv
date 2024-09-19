require 'helper'

describe Gitenv::Symlink::Action do
  subject { action }

  let(:from) { '/source' }
  let(:to) { File.expand_path '~' }
  let(:context) { OpenStruct.new from:, to: }
  let(:files) { double }
  let(:options) { {} }
  let(:action) { Gitenv::Symlink::Action.new context, files, options }

  it { is_expected.to be_a(Gitenv::Action) }

  context 'with custom options' do
    let(:options) { { overwrite: true, foo: :bar } }

    its(:options) { is_expected.to eq(overwrite: true, foo: :bar) }
  end

  context 'after calling overwrite' do
    subject { super().overwrite }

    it { is_expected.to be(action) }
    its(:options) { is_expected.to eq(overwrite: true) }
  end

  context 'after calling overwrite with false' do
    subject { super().overwrite false }

    it { is_expected.to be(action) }
    its(:options) { is_expected.to eq(overwrite: false) }
  end

  context 'after calling overwrite with the backup option set to false' do
    subject { super().overwrite backup: false }

    it { is_expected.to be(action) }
    its(:options) { is_expected.to eq(overwrite: true, backup: false) }
  end

  context 'after calling once' do
    subject { super().once }

    it { is_expected.to be(action) }
    its(:options) { is_expected.to eq(overwrite: false, backup: false) }
  end
end
