require 'helper'

describe Gitenv::Repository do
  subject{ described_class.new '/source' }

  its(:path){ should eq('/source') }

  it "should be equal to another repository at the same path" do
    expect(described_class.new(subject.path)).to eq(subject)
  end

  it "should be different than another repository at a different path" do
    expect(described_class.new("#{subject.path}/foo")).to_not eq(subject)
  end
end
