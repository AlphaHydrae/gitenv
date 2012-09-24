# Generated by jeweler
# DO NOT EDIT THIS FILE DIRECTLY
# Instead, edit Jeweler::Tasks in Rakefile, and run 'rake gemspec'
# -*- encoding: utf-8 -*-

Gem::Specification.new do |s|
  s.name = "gitenv"
  s.version = "0.0.5"

  s.required_rubygems_version = Gem::Requirement.new(">= 0") if s.respond_to? :required_rubygems_version=
  s.authors = ["AlphaHydrae"]
  s.date = "2012-09-24"
  s.description = "Gitenv sets up symlinks to your configuration files in a git repository."
  s.email = "hydrae.alpha@gmail.com"
  s.executables = ["gitenv"]
  s.extra_rdoc_files = [
    "LICENSE.txt",
    "README.md"
  ]
  s.files = [
    ".rspec",
    ".rvmrc",
    ".screenrc",
    ".travis.yml",
    "Gemfile",
    "Gemfile.lock",
    "LICENSE.txt",
    "README.md",
    "Rakefile",
    "VERSION",
    "bin/gitenv",
    "gitenv.gemspec",
    "lib/gitenv.rb",
    "lib/gitenv/action.rb",
    "lib/gitenv/config.rb",
    "lib/gitenv/context.rb",
    "lib/gitenv/controller.rb",
    "lib/gitenv/copy.rb",
    "lib/gitenv/enumerator.rb",
    "lib/gitenv/enumerator/all_files.rb",
    "lib/gitenv/enumerator/dot_files.rb",
    "lib/gitenv/enumerator/enumerator.rb",
    "lib/gitenv/enumerator/one_file.rb",
    "lib/gitenv/symlink.rb",
    "lib/program.rb",
    "spec/helper.rb",
    "spec/version_spec.rb"
  ]
  s.homepage = "http://github.com/AlphaHydrae/gitenv"
  s.licenses = ["MIT"]
  s.require_paths = ["lib"]
  s.rubygems_version = "1.8.24"
  s.summary = "Symlink manager for git repositories with configuration files."

  if s.respond_to? :specification_version then
    s.specification_version = 3

    if Gem::Version.new(Gem::VERSION) >= Gem::Version.new('1.2.0') then
      s.add_runtime_dependency(%q<paint>, ["~> 0.8.5"])
      s.add_runtime_dependency(%q<commander>, ["~> 4.1.2"])
      s.add_development_dependency(%q<bundler>, [">= 0"])
      s.add_development_dependency(%q<rake>, [">= 0"])
      s.add_development_dependency(%q<rspec>, [">= 0"])
      s.add_development_dependency(%q<jeweler>, [">= 0"])
      s.add_development_dependency(%q<gemcutter>, [">= 0"])
      s.add_development_dependency(%q<gem-release>, [">= 0"])
      s.add_development_dependency(%q<rake-version>, [">= 0"])
      s.add_development_dependency(%q<simplecov>, [">= 0"])
    else
      s.add_dependency(%q<paint>, ["~> 0.8.5"])
      s.add_dependency(%q<commander>, ["~> 4.1.2"])
      s.add_dependency(%q<bundler>, [">= 0"])
      s.add_dependency(%q<rake>, [">= 0"])
      s.add_dependency(%q<rspec>, [">= 0"])
      s.add_dependency(%q<jeweler>, [">= 0"])
      s.add_dependency(%q<gemcutter>, [">= 0"])
      s.add_dependency(%q<gem-release>, [">= 0"])
      s.add_dependency(%q<rake-version>, [">= 0"])
      s.add_dependency(%q<simplecov>, [">= 0"])
    end
  else
    s.add_dependency(%q<paint>, ["~> 0.8.5"])
    s.add_dependency(%q<commander>, ["~> 4.1.2"])
    s.add_dependency(%q<bundler>, [">= 0"])
    s.add_dependency(%q<rake>, [">= 0"])
    s.add_dependency(%q<rspec>, [">= 0"])
    s.add_dependency(%q<jeweler>, [">= 0"])
    s.add_dependency(%q<gemcutter>, [">= 0"])
    s.add_dependency(%q<gem-release>, [">= 0"])
    s.add_dependency(%q<rake-version>, [">= 0"])
    s.add_dependency(%q<simplecov>, [">= 0"])
  end
end

