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
    local file="Cargo.toml"
    if [[ -f $file ]]; then
        sed -i '' "s/^version = \".*\"/version = \"$version\"/" "$file"
    else
        echo "File $file does not exist."
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
    sleep 2
    create_version_commit "$version"
    create_and_push_git_tag "$version"
    echo "Created a new release $version"
else
    echo "Invalid version number. Please follow semver format (e.g., 1.0.0)."
    exit 1
fi
