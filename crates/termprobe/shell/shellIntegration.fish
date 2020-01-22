function __su_osc; printf '\033]133;%s\007' $argv[1]; end
function __su_cwd
    set -l path (string escape --style=url -- "$PWD" | string replace -a '%2F' '/' | string replace -a '%3A' ':')
    printf '\033]7;file://%s%s\007' "$hostname" "$path"
