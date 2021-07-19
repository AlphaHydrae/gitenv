#!/usr/bin/env bash
set -e

bump="$1"

echo "Bumping $bump version..."
rake "version:bump:$bump"
version="$(cat VERSION)"

echo "Updating gemspec..."
rake gemspec

tag="v${version}"
echo "Committing..."
git commit -m "$tag"

echo "Creating tag $tag..."
git tag "$tag"

echo "Pushing..."
git push --tags origin main
