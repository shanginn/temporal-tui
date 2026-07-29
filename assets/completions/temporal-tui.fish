# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_temporal_tui_global_optspecs
    string join \n config= profile= command-timeout= address= n/namespace= api-key= tls tls-ca= tls-cert= tls-key= tls-server-name= header= codec-endpoint= codec-header= codec-auth= q/query= page-size= refresh-seconds= no-auto-refresh no-color read-only web-ui-url= h/help V/version
end

function __fish_temporal_tui_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_temporal_tui_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_temporal_tui_using_subcommand
    set -l cmd (__fish_temporal_tui_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l address -d 'Temporal frontend address. A scheme is optional' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -s n -l namespace -d 'Namespace selected at startup' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l api-key -d 'Temporal Cloud API key. Prefer `profile set-api-key`' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l tls-ca -d 'PEM-encoded server CA certificate' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l tls-cert -d 'PEM-encoded mTLS client certificate' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l tls-key -d 'PEM-encoded mTLS client private key' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l tls-server-name -d 'TLS server name override' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l header -d 'Additional gRPC header in KEY=VALUE form. May be repeated' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l codec-endpoint -d 'Temporal Codec Server base URL; `{namespace}` is expanded in its path' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l codec-header -d 'Codec Server HTTP header in KEY=VALUE form. May be repeated' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l codec-auth -d 'Codec Server Authorization header. Prefer the environment variable' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -s q -l query -d 'Initial Temporal visibility query' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l page-size -d 'Workflows loaded per cursor page' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l refresh-seconds -d 'Automatic refresh interval in seconds' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l web-ui-url -d 'Base URL of Temporal Web UI' -r
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l tls -d 'Enable TLS even when the address does not use an https scheme'
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l no-auto-refresh -d 'Start with automatic refresh disabled'
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l no-color -d 'Disable colors while retaining text status labels'
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -l read-only -d 'Block every Temporal mutation, including signals'
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -s V -l version -d 'Print version'
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -f -a "profile" -d 'Manage connection profiles'
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -f -a "filter" -d 'Manage saved visibility queries'
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -f -a "auth" -d 'Sign in to a protected self-hosted Temporal deployment'
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -f -a "config-path" -d 'Print the active config path'
complete -c temporal-tui -n "__fish_temporal_tui_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -f -a "list" -d 'List configured profiles without resolving secrets'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -f -a "show" -d 'Print one redacted profile as TOML'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -f -a "create" -d 'Create a connection profile'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -f -a "set-default" -d 'Select the default profile'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -f -a "set-api-key" -d 'Read an API key without echo and store it in the OS credential manager'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -f -a "clear-api-key" -d 'Remove an API key reference and delete its OS credential'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -f -a "remove" -d 'Remove a profile'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and not __fish_seen_subcommand_from list show create set-default set-api-key clear-api-key remove help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from list" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from list" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from list" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from show" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from show" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from show" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from show" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l address -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -s n -l namespace -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l tls-ca -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l tls-cert -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l tls-key -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l tls-server-name -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l header -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l codec-endpoint -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l codec-header -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l codec-auth-env -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l api-key-env -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l web-ui-url -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l tls
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l read-only
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l set-default
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -l replace -d 'Replace an existing profile with the same name'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from set-default" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from set-default" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from set-default" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from set-default" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from set-api-key" -l from-env -d 'Read the API key from this environment variable instead of prompting' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from set-api-key" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from set-api-key" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from set-api-key" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from set-api-key" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from clear-api-key" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from clear-api-key" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from clear-api-key" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from clear-api-key" -l yes
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from clear-api-key" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from remove" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from remove" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from remove" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from remove" -l yes
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from remove" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from help" -f -a "list" -d 'List configured profiles without resolving secrets'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from help" -f -a "show" -d 'Print one redacted profile as TOML'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from help" -f -a "create" -d 'Create a connection profile'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from help" -f -a "set-default" -d 'Select the default profile'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from help" -f -a "set-api-key" -d 'Read an API key without echo and store it in the OS credential manager'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from help" -f -a "clear-api-key" -d 'Remove an API key reference and delete its OS credential'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from help" -f -a "remove" -d 'Remove a profile'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand profile; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and not __fish_seen_subcommand_from list save remove help" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and not __fish_seen_subcommand_from list save remove help" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and not __fish_seen_subcommand_from list save remove help" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and not __fish_seen_subcommand_from list save remove help" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and not __fish_seen_subcommand_from list save remove help" -f -a "list" -d 'List saved visibility queries'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and not __fish_seen_subcommand_from list save remove help" -f -a "save" -d 'Save or replace a visibility query'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and not __fish_seen_subcommand_from list save remove help" -f -a "remove" -d 'Remove a saved visibility query'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and not __fish_seen_subcommand_from list save remove help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from list" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from list" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from list" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from save" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from save" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from save" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from save" -l replace
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from save" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from remove" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from remove" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from remove" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from remove" -l yes
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from remove" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from help" -f -a "list" -d 'List saved visibility queries'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from help" -f -a "save" -d 'Save or replace a visibility query'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from help" -f -a "remove" -d 'Remove a saved visibility query'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand filter; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and not __fish_seen_subcommand_from login whoami logout help" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and not __fish_seen_subcommand_from login whoami logout help" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and not __fish_seen_subcommand_from login whoami logout help" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and not __fish_seen_subcommand_from login whoami logout help" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and not __fish_seen_subcommand_from login whoami logout help" -f -a "login" -d 'Sign in with a local username and a password read without echo'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and not __fish_seen_subcommand_from login whoami logout help" -f -a "whoami" -d 'Show the current signed-in identity and session status'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and not __fish_seen_subcommand_from login whoami logout help" -f -a "logout" -d 'Revoke the refresh grant and remove its local credential'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and not __fish_seen_subcommand_from login whoami logout help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -l url -d 'Temporal auth base URL' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -l username -d 'Local username. Prompted when omitted' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -l address -d 'Temporal gRPC address override when the auth service does not advertise one' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -s n -l namespace -d 'Namespace stored in a newly created profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -l password-stdin -d 'Read the password from stdin instead of a terminal prompt'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -l allow-http -d 'Permit loopback-only HTTP for local development and tests'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -l set-default -d 'Make the selected profile the default'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from login" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from whoami" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from whoami" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from whoami" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from whoami" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from logout" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from logout" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from logout" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from logout" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from help" -f -a "login" -d 'Sign in with a local username and a password read without echo'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from help" -f -a "whoami" -d 'Show the current signed-in identity and session status'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from help" -f -a "logout" -d 'Revoke the refresh grant and remove its local credential'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand auth; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand config-path" -l config -d 'Alternate config file. Defaults to the platform user config directory' -r -F
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand config-path" -l profile -d 'Named connection profile' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand config-path" -l command-timeout -d 'Temporal CLI forwards this host-enforced timeout to extensions' -r
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand config-path" -s h -l help -d 'Print help'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and not __fish_seen_subcommand_from profile filter auth config-path help" -f -a "profile" -d 'Manage connection profiles'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and not __fish_seen_subcommand_from profile filter auth config-path help" -f -a "filter" -d 'Manage saved visibility queries'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and not __fish_seen_subcommand_from profile filter auth config-path help" -f -a "auth" -d 'Sign in to a protected self-hosted Temporal deployment'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and not __fish_seen_subcommand_from profile filter auth config-path help" -f -a "config-path" -d 'Print the active config path'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and not __fish_seen_subcommand_from profile filter auth config-path help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from profile" -f -a "list" -d 'List configured profiles without resolving secrets'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from profile" -f -a "show" -d 'Print one redacted profile as TOML'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from profile" -f -a "create" -d 'Create a connection profile'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from profile" -f -a "set-default" -d 'Select the default profile'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from profile" -f -a "set-api-key" -d 'Read an API key without echo and store it in the OS credential manager'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from profile" -f -a "clear-api-key" -d 'Remove an API key reference and delete its OS credential'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from profile" -f -a "remove" -d 'Remove a profile'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from filter" -f -a "list" -d 'List saved visibility queries'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from filter" -f -a "save" -d 'Save or replace a visibility query'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from filter" -f -a "remove" -d 'Remove a saved visibility query'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from auth" -f -a "login" -d 'Sign in with a local username and a password read without echo'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from auth" -f -a "whoami" -d 'Show the current signed-in identity and session status'
complete -c temporal-tui -n "__fish_temporal_tui_using_subcommand help; and __fish_seen_subcommand_from auth" -f -a "logout" -d 'Revoke the refresh grant and remove its local credential'
