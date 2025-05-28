#!/bin/bash

format_rust_files() {
        file="$1"
        extension="${file##*.}"
        if [[ $extension == "rs" ]]; then
                echo "Formatting: $file"
                rustfmt "$file" --skip-children --unstable-features 
        fi
}

stage_files() {
        file="$1"
        git add "$file" 
}


main() {
        repo_root=$(git rev-parse --show-toplevel)  # Get absolute path to repo root
        files=$(git diff --cached --name-only --diff-filter=d)


        for staged_file in $files 
        do
                format_rust_files "$staged_file" 
        done

        for staged_file in $files 
        do
                stage_files "$staged_file"
        done
}

main
