require 'helper'

describe 'Version' do
  it 'is correct' do
    version_file = File.join File.dirname(__FILE__), '..', 'VERSION'
    expect(Gitenv::VERSION).to eq(File.open(version_file, 'r').read)
  end
end
