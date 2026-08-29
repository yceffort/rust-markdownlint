# fish completion for rust-markdownlint.
# Install as ~/.config/fish/completions/rust-markdownlint.fish:
#   rust-markdownlint completions fish > ~/.config/fish/completions/rust-markdownlint.fish

function __rust_markdownlint_markdown
    __fish_complete_suffix .md
    __fish_complete_suffix .markdown
end

function __rust_markdownlint_config
    __fish_complete_suffix .jsonc
    __fish_complete_suffix .json
    __fish_complete_suffix .toml
    __fish_complete_suffix .yaml
    __fish_complete_suffix .yml
end

complete -c rust-markdownlint -f
complete -c rust-markdownlint -n __fish_use_subcommand -a completions -d 'write a shell completion script to stdout'
complete -c rust-markdownlint -n __fish_use_subcommand -a server -d 'run a Language Server Protocol server on stdio'
complete -c rust-markdownlint -n '__fish_seen_subcommand_from completions' -a 'bash zsh fish' -d shell

complete -c rust-markdownlint -n 'not __fish_seen_subcommand_from completions server' -a '(__rust_markdownlint_markdown)' -d 'markdown file'
complete -c rust-markdownlint -n 'not __fish_seen_subcommand_from completions server' -l config -r -a '(__rust_markdownlint_config)' -d 'configuration file for the base configuration'
complete -c rust-markdownlint -n 'not __fish_seen_subcommand_from completions server' -l configPointer -r -d 'JSON Pointer into the --config file'
complete -c rust-markdownlint -n 'not __fish_seen_subcommand_from completions server' -l diff -d 'write what --fix would change to stdout as a unified diff'
complete -c rust-markdownlint -n 'not __fish_seen_subcommand_from completions server' -l fix -d 'update files to resolve fixable issues'
complete -c rust-markdownlint -n 'not __fish_seen_subcommand_from completions server' -l format -d 'read stdin, apply fixes, write stdout'
complete -c rust-markdownlint -n 'not __fish_seen_subcommand_from completions server' -l help -d 'write the help message and exit'
complete -c rust-markdownlint -n 'not __fish_seen_subcommand_from completions server' -l no-globs -d 'ignore the globs property in the configuration'
complete -c rust-markdownlint -n 'not __fish_seen_subcommand_from completions server' -l stdin-filename -r -a '(__rust_markdownlint_markdown)' -d 'name stdin as if it were the given file path'
