require 'helper'

describe Gitenv::Config do
  subject{ Gitenv::Config.new }

  its(:repos){ should be_empty }
  its(:actions){ should be_empty }
end
