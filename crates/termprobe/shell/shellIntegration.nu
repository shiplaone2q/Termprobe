# Keep Nushell's native semantic-prompt integration, but emit OSC 7 ourselves
# to encode literal percent signs and other URI-reserved characters in paths.
$env.config = ($env.config | default {})
$env.config.shell_integration = ($env.config.shell_integration | default {})
$env.config.shell_integration.osc133 = true
$env.config.shell_integration.osc7 = false
$env.config.show_banner = false

def __su_cwd [] {
    let path = if $nu.os-info.name == "windows" {
        $env.PWD | str replace --all '\' '/'
    } else {
