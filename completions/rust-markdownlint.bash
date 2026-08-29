# bash completion for rust-markdownlint.
# Install as /etc/bash_completion.d/rust-markdownlint, or source it:
#   source <(rust-markdownlint completions bash)

_rust_markdownlint_suffix() {
    local cur="$1" ext
    shift
    for ext in "$@"; do
        COMPREPLY+=($(compgen -f -X "!*.$ext" -- "$cur"))
    done
    COMPREPLY+=($(compgen -d -- "$cur"))
}

_rust_markdownlint() {
    local cur prev
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD - 1]}"

    case "$prev" in
        --config)
            # 지원되는 설정 파일 이름은 전부 이 확장자로 끝나므로 접미사 완성이 그대로 덮는다
            _rust_markdownlint_suffix "$cur" jsonc json toml yaml yml
            return
            ;;
        --configPointer)
            return
            ;;
        --stdin-filename)
            _rust_markdownlint_suffix "$cur" md markdown
            return
            ;;
        completions)
            COMPREPLY=($(compgen -W "bash zsh fish" -- "$cur"))
            return
            ;;
    esac

    if [ "${cur:0:1}" = "-" ]; then
        COMPREPLY=($(compgen -W "--config --configPointer --diff --fix --format --help --no-globs --stdin-filename" -- "$cur"))
        return
    fi
    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=($(compgen -W "completions server" -- "$cur"))
    fi
    _rust_markdownlint_suffix "$cur" md markdown
}

complete -o filenames -F _rust_markdownlint rust-markdownlint
