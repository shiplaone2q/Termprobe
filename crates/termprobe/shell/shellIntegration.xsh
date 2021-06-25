import pathlib as __su_pathlib

$XONSH_SUPPRESS_WELCOME = True

def __su_osc(m):
    print(f'\033]133;{m}\007', end='', flush=True)

def __su_cwd():
    return f'\033]7;{__su_pathlib.Path.cwd().as_uri()}\007'

@events.on_precommand
def __su_pre(cmd, **kw):
    __su_osc('C')

