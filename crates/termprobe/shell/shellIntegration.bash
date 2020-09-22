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
