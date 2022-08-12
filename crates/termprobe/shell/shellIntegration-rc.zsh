builtin autoload -Uz add-zsh-hook

__su_zdotdir=$ZDOTDIR
if [[ -f $USER_ZDOTDIR/.zshrc ]]; then
	ZDOTDIR=$USER_ZDOTDIR
	. "$USER_ZDOTDIR/.zshrc"
fi
ZDOTDIR=$__su_zdotdir
