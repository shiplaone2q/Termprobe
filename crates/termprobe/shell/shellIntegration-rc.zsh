builtin autoload -Uz add-zsh-hook

__su_zdotdir=$ZDOTDIR
if [[ -f $USER_ZDOTDIR/.zshrc ]]; then
	ZDOTDIR=$USER_ZDOTDIR
	. "$USER_ZDOTDIR/.zshrc"
fi
ZDOTDIR=$__su_zdotdir

__su_osc() { builtin printf '\033]133;%s\007' "$1"; }
__su_cwd() {
	emulate -L zsh
