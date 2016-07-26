require 'helper'

describe Gitenv::Symlink::Action do

  let(:from){ '/source' }
  let(:to){ File.expand_path '~' }
  let(:context){ OpenStruct.new from: from, to: to }
  let(:files){ double }
  let(:options){ {} }
  let(:action){ Gitenv::Symlink::Action.new context, files, options }
  subject{ action }

  it{ should be_a_kind_of(Gitenv::Action) }

  context "with custom options" do
    let(:options){ { overwrite: true, foo: :bar } }
    its(:options){ should eq(overwrite: true, foo: :bar) }
  end

  context "after calling overwrite" do
    subject{ super().overwrite }
    it{ should be(action) }
    its(:options){ should eq(overwrite: true) }
  end

  context "after calling overwrite with false" do
    subject{ super().overwrite false }
    it{ should be(action) }
    its(:options){ should eq(overwrite: false) }
  end

  context "after calling overwrite with the backup option set to false" do
    subject{ super().overwrite backup: false }
    it{ should be(action) }
    its(:options){ should eq(overwrite: true, backup: false) }
  end

  context "after calling once" do
    subject{ super().once }
    it{ should be(action) }
    its(:options){ should eq(overwrite: false, backup: false) }
  end
end
