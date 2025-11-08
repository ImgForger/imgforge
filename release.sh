#!/bin/bash

# Validate semver
validate_semver() {
    local version=$1
    if [[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        return 0
    else
        return 1
    fi
}

# Check if the git repository is clean
check_git_clean() {
    if [[ -n $(git status --porcelain) ]]; then
        echo "Git repository has uncommitted changes. Please commit or stash them before proceeding."
        exit 1
    fi
}

# Check if the current branch is 'main'
check_git_branch() {
    local branch
    branch=$(git symbolic-ref --short HEAD)
    if [[ $branch != "main" ]]; then
        echo "You are not on the 'main' branch. Please switch to the 'main' branch before proceeding."
        exit 1
    fi
}

# Update version in Cargo.toml
update_version_file() {
    local version=$1
    local toml_file="Cargo.toml"
    local lock_file="Cargo.lock"

    if [[ -f $toml_file ]]; then
        sed -i '' "s/^version = \".*\"/version = \"$version\"/" "$toml_file"
    else
        echo "File $toml_file does not exist."
        exit 1
    fi

    if [[ -f $lock_file ]]; then
        python3 - "$version" "$lock_file" <<'PY'
import pathlib
import re
import sys

version = sys.argv[1]
lock_path = pathlib.Path(sys.argv[2])
text = lock_path.read_text()
pattern = r'(\[\[package\]\]\nname = "imgforge"\nversion = )"[^"]+"'
new_text, count = re.subn(pattern, r'\1"' + version + '"', text, count=1)
if count == 0:
    print("Failed to update version in Cargo.lock: could not find imgforge package.", file=sys.stderr)
    sys.exit(1)
lock_path.write_text(new_text)
PY
    else
        echo "File $lock_file does not exist."
        exit 1
    fi
    sleep 2
}

# Commit and push the changes
create_version_commit() {
    local version=$1
    git add Cargo.toml Cargo.lock
    git commit -m "Release $version"
    git push origin main
}

# Create a new git tag and push it
create_and_push_git_tag() {
    local version=$1
    local tag="v$version"
    git tag "$tag"
    git push origin "$tag"
}

# Main script
check_git_clean
check_git_branch
read -r -p "Enter the version number: " version

if validate_semver "$version"; then
    update_version_file "$version"
    create_version_commit "$version"
    create_and_push_git_tag "$version"
    echo "Created a new release $version"
else
    echo "Invalid version number. Please follow semver format (e.g., 1.0.0)."
    exit 1
fi
