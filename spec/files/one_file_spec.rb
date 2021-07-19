# frozen_string_literal: true

require 'helper'

describe Gitenv::OneFile, fakefs: true do
  subject{ matcher.files(source) }

  let(:matcher){ described_class.new file }
  let(:source){ '/source' }
  let(:files){ ['file.txt', '.dotfile', 'README'] }
  let(:file){ 'README' }

  before do
    FileUtils.mkdir_p source
    files.each{ |f| FileUtils.touch File.join(source, f) }
  end

  it{ is_expected.to match_array([file]) }
end
