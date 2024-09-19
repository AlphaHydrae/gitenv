%i[matcher all_files dot_files one_file].each do |lib|
  require File.join(File.dirname(__FILE__), 'files', lib.to_s)
end
