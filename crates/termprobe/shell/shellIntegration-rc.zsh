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
	local LC_ALL=C p=$PWD encoded='' byte c hex i
	for ((i = 1; i <= ${#p}; i++)); do
		c=$p[i]
		case "$c" in
			[a-zA-Z0-9/._~:-]) encoded+=$c ;;
			*)
				builtin printf -v byte '%d' "'$c"
				builtin printf -v hex '%%%02X' "$((byte & 255))"
				encoded+=$hex
