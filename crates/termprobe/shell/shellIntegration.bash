if [ -r ~/.bashrc ]; then
    . ~/.bashrc
fi
if [ -r /etc/profile ]; then
    . /etc/profile
fi
if [ -r ~/.bash_profile ]; then
    . ~/.bash_profile
elif [ -r ~/.bash_login ]; then
    . ~/.bash_login
elif [ -r ~/.profile ]; then
    . ~/.profile
fi

__su_osc() { builtin printf '\033]133;%s\007' "$1"; }
__su_cwd() {
    # Iterate over UTF-8 bytes, not characters, when percent-encoding the URI.
    local LC_ALL=C p=$PWD encoded='' byte c hex i
    for ((i = 0; i < ${#p}; i++)); do
        c=${p:i:1}
        case "$c" in
            [a-zA-Z0-9/._~:-]) encoded+=$c ;;
            *)
                builtin printf -v byte '%d' "'$c"
                builtin printf -v hex '%%%02X' "$((byte & 255))"
                encoded+=$hex
                ;;
        esac
    done
