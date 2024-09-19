RSpec::Matchers.define :eq_unpainted do |expected|
  match do |actual|
    Paint.unpaint(actual) == expected
  end

  description do
    %/eq (unpainted) "#{expected}"/
  end
end

RSpec::Matchers.define :link_to do |expected|
  match do |actual|
    File.expand_path(File.readlink(actual)) == File.expand_path(expected)
  end
end

RSpec::Matchers.define :exist do
  match do |actual|
    File.exist? actual
  end
end

RSpec::Matchers.define :contain do |expected|
  match do |actual|
    File.read(actual) == expected
  end

  failure_message do |actual|
    "expected that #{actual} would contain #{contents expected}"
  end

  failure_message_when_negated do |actual|
    "expected that #{actual} would not contain #{contents expected}"
  end

  description do
    "contain #{contents expected}"
  end

  def contents(c)
    c.length > 20 ? %("#{c[0, 17]}...") : %("#{c}")
  end
end

RSpec::Matchers.define :have_changed do |expected|
  match do |actual|
    File.mtime(actual) != expected
  end

  failure_message do |actual|
    "expected that #{actual} would have changed since #{expected}"
  end

  failure_message_when_negated do |actual|
    "expected that #{actual} would not have changed since #{expected}"
  end

  description do
    'have changed'
  end
end

RSpec::Matchers.define :be_status do |expected, *message|
  match do |actual|
    @expected_type = expected
    @expected_messages = message
    @type_matches = actual.type == expected
    @message_matches = @expected_messages.all? { |m| actual.message.match(m) }
    @type_matches and @message_matches
  end

  failure_message do |actual|
    "expected that #{actual.inspect} would #{failures.join ' and '}"
  end

  failure_message_when_negated do |actual|
    "expected that #{actual.inspect} would not be of type :#{@expected_type} and have a message matching #{@expected_messages.inspect}"
  end

  description do
    "be :#{expected} with a message matching #{message.inspect}"
  end

  def failures
    [].tap do |a|
      a << "be of type :#{@expected_type}" unless @type_matches
      a << "have a message matching #{@expected_messages.inspect}" unless @message_matches
    end
  end
end
