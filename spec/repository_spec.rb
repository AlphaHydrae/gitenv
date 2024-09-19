require 'helper'

describe Gitenv::Repository do
  subject { described_class.new '/source' }

  its(:path) { is_expected.to eq('/source') }

  it 'is equal to another repository at the same path' do
    expect(described_class.new(subject.path)).to eq(subject)
  end

  it 'is different than another repository at a different path' do
    expect(described_class.new("#{subject.path}/foo")).not_to eq(subject)
  end
end
