
use builtin;
use str;

set edit:completion:arg-completer[temporal-tui] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'temporal-tui'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'temporal-tui'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand --address 'Temporal frontend address. A scheme is optional'
            cand -n 'Namespace selected at startup'
            cand --namespace 'Namespace selected at startup'
            cand --api-key 'Temporal Cloud API key. Prefer `profile set-api-key`'
            cand --tls-ca 'PEM-encoded server CA certificate'
            cand --tls-cert 'PEM-encoded mTLS client certificate'
            cand --tls-key 'PEM-encoded mTLS client private key'
            cand --tls-server-name 'TLS server name override'
            cand --header 'Additional gRPC header in KEY=VALUE form. May be repeated'
            cand --codec-endpoint 'Temporal Codec Server base URL; `{namespace}` is expanded in its path'
            cand --codec-header 'Codec Server HTTP header in KEY=VALUE form. May be repeated'
            cand --codec-auth 'Codec Server Authorization header. Prefer the environment variable'
            cand -q 'Initial Temporal visibility query'
            cand --query 'Initial Temporal visibility query'
            cand --page-size 'Workflows loaded per cursor page'
            cand --refresh-seconds 'Automatic refresh interval in seconds'
            cand --web-ui-url 'Base URL of Temporal Web UI'
            cand --tls 'Enable TLS even when the address does not use an https scheme'
            cand --no-auto-refresh 'Start with automatic refresh disabled'
            cand --no-color 'Disable colors while retaining text status labels'
            cand --read-only 'Block every Temporal mutation, including signals'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand profile 'Manage connection profiles'
            cand filter 'Manage saved visibility queries'
            cand auth 'Sign in to a protected self-hosted Temporal deployment'
            cand config-path 'Print the active config path'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'temporal-tui;profile'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
            cand list 'List configured profiles without resolving secrets'
            cand show 'Print one redacted profile as TOML'
            cand create 'Create a connection profile'
            cand set-default 'Select the default profile'
            cand set-api-key 'Read an API key without echo and store it in the OS credential manager'
            cand clear-api-key 'Remove an API key reference and delete its OS credential'
            cand remove 'Remove a profile'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'temporal-tui;profile;list'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;profile;show'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;profile;create'= {
            cand --address 'address'
            cand -n 'n'
            cand --namespace 'namespace'
            cand --tls-ca 'tls-ca'
            cand --tls-cert 'tls-cert'
            cand --tls-key 'tls-key'
            cand --tls-server-name 'tls-server-name'
            cand --header 'header'
            cand --codec-endpoint 'codec-endpoint'
            cand --codec-header 'codec-header'
            cand --codec-auth-env 'codec-auth-env'
            cand --api-key-env 'api-key-env'
            cand --web-ui-url 'web-ui-url'
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand --tls 'tls'
            cand --read-only 'read-only'
            cand --set-default 'set-default'
            cand --replace 'Replace an existing profile with the same name'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;profile;set-default'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;profile;set-api-key'= {
            cand --from-env 'Read the API key from this environment variable instead of prompting'
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;profile;clear-api-key'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand --yes 'yes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;profile;remove'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand --yes 'yes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;profile;help'= {
            cand list 'List configured profiles without resolving secrets'
            cand show 'Print one redacted profile as TOML'
            cand create 'Create a connection profile'
            cand set-default 'Select the default profile'
            cand set-api-key 'Read an API key without echo and store it in the OS credential manager'
            cand clear-api-key 'Remove an API key reference and delete its OS credential'
            cand remove 'Remove a profile'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'temporal-tui;profile;help;list'= {
        }
        &'temporal-tui;profile;help;show'= {
        }
        &'temporal-tui;profile;help;create'= {
        }
        &'temporal-tui;profile;help;set-default'= {
        }
        &'temporal-tui;profile;help;set-api-key'= {
        }
        &'temporal-tui;profile;help;clear-api-key'= {
        }
        &'temporal-tui;profile;help;remove'= {
        }
        &'temporal-tui;profile;help;help'= {
        }
        &'temporal-tui;filter'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
            cand list 'List saved visibility queries'
            cand save 'Save or replace a visibility query'
            cand remove 'Remove a saved visibility query'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'temporal-tui;filter;list'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;filter;save'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand --replace 'replace'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;filter;remove'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand --yes 'yes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;filter;help'= {
            cand list 'List saved visibility queries'
            cand save 'Save or replace a visibility query'
            cand remove 'Remove a saved visibility query'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'temporal-tui;filter;help;list'= {
        }
        &'temporal-tui;filter;help;save'= {
        }
        &'temporal-tui;filter;help;remove'= {
        }
        &'temporal-tui;filter;help;help'= {
        }
        &'temporal-tui;auth'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
            cand login 'Sign in with a local username and a password read without echo'
            cand whoami 'Show the current signed-in identity and session status'
            cand logout 'Revoke the refresh grant and remove its local credential'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'temporal-tui;auth;login'= {
            cand --url 'Temporal auth base URL'
            cand --username 'Local username. Prompted when omitted'
            cand --address 'Temporal gRPC address override when the auth service does not advertise one'
            cand -n 'Namespace stored in a newly created profile'
            cand --namespace 'Namespace stored in a newly created profile'
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand --password-stdin 'Read the password from stdin instead of a terminal prompt'
            cand --allow-http 'Permit loopback-only HTTP for local development and tests'
            cand --set-default 'Make the selected profile the default'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;auth;whoami'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;auth;logout'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;auth;help'= {
            cand login 'Sign in with a local username and a password read without echo'
            cand whoami 'Show the current signed-in identity and session status'
            cand logout 'Revoke the refresh grant and remove its local credential'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'temporal-tui;auth;help;login'= {
        }
        &'temporal-tui;auth;help;whoami'= {
        }
        &'temporal-tui;auth;help;logout'= {
        }
        &'temporal-tui;auth;help;help'= {
        }
        &'temporal-tui;config-path'= {
            cand --config 'Alternate config file. Defaults to the platform user config directory'
            cand --profile 'Named connection profile'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'temporal-tui;help'= {
            cand profile 'Manage connection profiles'
            cand filter 'Manage saved visibility queries'
            cand auth 'Sign in to a protected self-hosted Temporal deployment'
            cand config-path 'Print the active config path'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'temporal-tui;help;profile'= {
            cand list 'List configured profiles without resolving secrets'
            cand show 'Print one redacted profile as TOML'
            cand create 'Create a connection profile'
            cand set-default 'Select the default profile'
            cand set-api-key 'Read an API key without echo and store it in the OS credential manager'
            cand clear-api-key 'Remove an API key reference and delete its OS credential'
            cand remove 'Remove a profile'
        }
        &'temporal-tui;help;profile;list'= {
        }
        &'temporal-tui;help;profile;show'= {
        }
        &'temporal-tui;help;profile;create'= {
        }
        &'temporal-tui;help;profile;set-default'= {
        }
        &'temporal-tui;help;profile;set-api-key'= {
        }
        &'temporal-tui;help;profile;clear-api-key'= {
        }
        &'temporal-tui;help;profile;remove'= {
        }
        &'temporal-tui;help;filter'= {
            cand list 'List saved visibility queries'
            cand save 'Save or replace a visibility query'
            cand remove 'Remove a saved visibility query'
        }
        &'temporal-tui;help;filter;list'= {
        }
        &'temporal-tui;help;filter;save'= {
        }
        &'temporal-tui;help;filter;remove'= {
        }
        &'temporal-tui;help;auth'= {
            cand login 'Sign in with a local username and a password read without echo'
            cand whoami 'Show the current signed-in identity and session status'
            cand logout 'Revoke the refresh grant and remove its local credential'
        }
        &'temporal-tui;help;auth;login'= {
        }
        &'temporal-tui;help;auth;whoami'= {
        }
        &'temporal-tui;help;auth;logout'= {
        }
        &'temporal-tui;help;config-path'= {
        }
        &'temporal-tui;help;help'= {
        }
    ]
    $completions[$command]
}
