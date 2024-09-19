require 'helper'

describe 'Digest' do
  DIGESTS = {
    1024 => '32bb048d0c2051b75e88f7697cb639ee0a003adb',
    4096 => '3bc354d1386768c45695dfd7af88b56d2bb70286',
    5120 => '67d76e60cceb68e0f4b556660ea7a1874bcbfb4a',
    9216 => '9f77974fb9bb00e7641a4b0787742ffd85261f78'
  }

  subject { Gitenv::Copy.new OpenStruct.new(from: '/source', to: File.expand_path('~')), '.zshlogin' }

  DIGESTS.each_key do |bytes|
    it "correctlies compute the SHA1 of a file with #{bytes} bytes" do
      file = File.join File.dirname(__FILE__), '..', 'fixtures', "file_with_#{bytes}_bytes"
      expect(subject.send(:digest, file)).to eq(DIGESTS[bytes])
    end
  end
end
