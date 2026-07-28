
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'temporal-tui' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'temporal-tui'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'temporal-tui' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('--address', '--address', [CompletionResultType]::ParameterName, 'Temporal frontend address. A scheme is optional')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'Namespace selected at startup')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'Namespace selected at startup')
            [CompletionResult]::new('--api-key', '--api-key', [CompletionResultType]::ParameterName, 'Temporal Cloud API key. Prefer `profile set-api-key`')
            [CompletionResult]::new('--tls-ca', '--tls-ca', [CompletionResultType]::ParameterName, 'PEM-encoded server CA certificate')
            [CompletionResult]::new('--tls-cert', '--tls-cert', [CompletionResultType]::ParameterName, 'PEM-encoded mTLS client certificate')
            [CompletionResult]::new('--tls-key', '--tls-key', [CompletionResultType]::ParameterName, 'PEM-encoded mTLS client private key')
            [CompletionResult]::new('--tls-server-name', '--tls-server-name', [CompletionResultType]::ParameterName, 'TLS server name override')
            [CompletionResult]::new('--header', '--header', [CompletionResultType]::ParameterName, 'Additional gRPC header in KEY=VALUE form. May be repeated')
            [CompletionResult]::new('--codec-endpoint', '--codec-endpoint', [CompletionResultType]::ParameterName, 'Temporal Codec Server base URL; `{namespace}` is expanded in its path')
            [CompletionResult]::new('--codec-header', '--codec-header', [CompletionResultType]::ParameterName, 'Codec Server HTTP header in KEY=VALUE form. May be repeated')
            [CompletionResult]::new('--codec-auth', '--codec-auth', [CompletionResultType]::ParameterName, 'Codec Server Authorization header. Prefer the environment variable')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Initial Temporal visibility query')
            [CompletionResult]::new('--query', '--query', [CompletionResultType]::ParameterName, 'Initial Temporal visibility query')
            [CompletionResult]::new('--page-size', '--page-size', [CompletionResultType]::ParameterName, 'Workflows loaded per cursor page')
            [CompletionResult]::new('--refresh-seconds', '--refresh-seconds', [CompletionResultType]::ParameterName, 'Automatic refresh interval in seconds')
            [CompletionResult]::new('--web-ui-url', '--web-ui-url', [CompletionResultType]::ParameterName, 'Base URL of Temporal Web UI')
            [CompletionResult]::new('--tls', '--tls', [CompletionResultType]::ParameterName, 'Enable TLS even when the address does not use an https scheme')
            [CompletionResult]::new('--no-auto-refresh', '--no-auto-refresh', [CompletionResultType]::ParameterName, 'Start with automatic refresh disabled')
            [CompletionResult]::new('--no-color', '--no-color', [CompletionResultType]::ParameterName, 'Disable colors while retaining text status labels')
            [CompletionResult]::new('--read-only', '--read-only', [CompletionResultType]::ParameterName, 'Block every Temporal mutation, including signals')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('profile', 'profile', [CompletionResultType]::ParameterValue, 'Manage connection profiles')
            [CompletionResult]::new('filter', 'filter', [CompletionResultType]::ParameterValue, 'Manage saved visibility queries')
            [CompletionResult]::new('auth', 'auth', [CompletionResultType]::ParameterValue, 'Sign in to a protected self-hosted Temporal deployment')
            [CompletionResult]::new('config-path', 'config-path', [CompletionResultType]::ParameterValue, 'Print the active config path')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'temporal-tui;profile' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List configured profiles without resolving secrets')
            [CompletionResult]::new('show', 'show', [CompletionResultType]::ParameterValue, 'Print one redacted profile as TOML')
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create a connection profile')
            [CompletionResult]::new('set-default', 'set-default', [CompletionResultType]::ParameterValue, 'Select the default profile')
            [CompletionResult]::new('set-api-key', 'set-api-key', [CompletionResultType]::ParameterValue, 'Read an API key without echo and store it in the OS credential manager')
            [CompletionResult]::new('clear-api-key', 'clear-api-key', [CompletionResultType]::ParameterValue, 'Remove an API key reference and delete its OS credential')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a profile')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'temporal-tui;profile;list' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;profile;show' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;profile;create' {
            [CompletionResult]::new('--address', '--address', [CompletionResultType]::ParameterName, 'address')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'n')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'namespace')
            [CompletionResult]::new('--tls-ca', '--tls-ca', [CompletionResultType]::ParameterName, 'tls-ca')
            [CompletionResult]::new('--tls-cert', '--tls-cert', [CompletionResultType]::ParameterName, 'tls-cert')
            [CompletionResult]::new('--tls-key', '--tls-key', [CompletionResultType]::ParameterName, 'tls-key')
            [CompletionResult]::new('--tls-server-name', '--tls-server-name', [CompletionResultType]::ParameterName, 'tls-server-name')
            [CompletionResult]::new('--header', '--header', [CompletionResultType]::ParameterName, 'header')
            [CompletionResult]::new('--codec-endpoint', '--codec-endpoint', [CompletionResultType]::ParameterName, 'codec-endpoint')
            [CompletionResult]::new('--codec-header', '--codec-header', [CompletionResultType]::ParameterName, 'codec-header')
            [CompletionResult]::new('--codec-auth-env', '--codec-auth-env', [CompletionResultType]::ParameterName, 'codec-auth-env')
            [CompletionResult]::new('--api-key-env', '--api-key-env', [CompletionResultType]::ParameterName, 'api-key-env')
            [CompletionResult]::new('--web-ui-url', '--web-ui-url', [CompletionResultType]::ParameterName, 'web-ui-url')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('--tls', '--tls', [CompletionResultType]::ParameterName, 'tls')
            [CompletionResult]::new('--read-only', '--read-only', [CompletionResultType]::ParameterName, 'read-only')
            [CompletionResult]::new('--set-default', '--set-default', [CompletionResultType]::ParameterName, 'set-default')
            [CompletionResult]::new('--replace', '--replace', [CompletionResultType]::ParameterName, 'Replace an existing profile with the same name')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;profile;set-default' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;profile;set-api-key' {
            [CompletionResult]::new('--from-env', '--from-env', [CompletionResultType]::ParameterName, 'Read the API key from this environment variable instead of prompting')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;profile;clear-api-key' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('--yes', '--yes', [CompletionResultType]::ParameterName, 'yes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;profile;remove' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('--yes', '--yes', [CompletionResultType]::ParameterName, 'yes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;profile;help' {
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List configured profiles without resolving secrets')
            [CompletionResult]::new('show', 'show', [CompletionResultType]::ParameterValue, 'Print one redacted profile as TOML')
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create a connection profile')
            [CompletionResult]::new('set-default', 'set-default', [CompletionResultType]::ParameterValue, 'Select the default profile')
            [CompletionResult]::new('set-api-key', 'set-api-key', [CompletionResultType]::ParameterValue, 'Read an API key without echo and store it in the OS credential manager')
            [CompletionResult]::new('clear-api-key', 'clear-api-key', [CompletionResultType]::ParameterValue, 'Remove an API key reference and delete its OS credential')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a profile')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'temporal-tui;profile;help;list' {
            break
        }
        'temporal-tui;profile;help;show' {
            break
        }
        'temporal-tui;profile;help;create' {
            break
        }
        'temporal-tui;profile;help;set-default' {
            break
        }
        'temporal-tui;profile;help;set-api-key' {
            break
        }
        'temporal-tui;profile;help;clear-api-key' {
            break
        }
        'temporal-tui;profile;help;remove' {
            break
        }
        'temporal-tui;profile;help;help' {
            break
        }
        'temporal-tui;filter' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List saved visibility queries')
            [CompletionResult]::new('save', 'save', [CompletionResultType]::ParameterValue, 'Save or replace a visibility query')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a saved visibility query')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'temporal-tui;filter;list' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;filter;save' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('--replace', '--replace', [CompletionResultType]::ParameterName, 'replace')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;filter;remove' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('--yes', '--yes', [CompletionResultType]::ParameterName, 'yes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;filter;help' {
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List saved visibility queries')
            [CompletionResult]::new('save', 'save', [CompletionResultType]::ParameterValue, 'Save or replace a visibility query')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a saved visibility query')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'temporal-tui;filter;help;list' {
            break
        }
        'temporal-tui;filter;help;save' {
            break
        }
        'temporal-tui;filter;help;remove' {
            break
        }
        'temporal-tui;filter;help;help' {
            break
        }
        'temporal-tui;auth' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('login', 'login', [CompletionResultType]::ParameterValue, 'Sign in with a local username and a password read without echo')
            [CompletionResult]::new('whoami', 'whoami', [CompletionResultType]::ParameterValue, 'Show the current signed-in identity and session status')
            [CompletionResult]::new('logout', 'logout', [CompletionResultType]::ParameterValue, 'Revoke the refresh grant and remove its local credential')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'temporal-tui;auth;login' {
            [CompletionResult]::new('--url', '--url', [CompletionResultType]::ParameterName, 'Temporal auth base URL')
            [CompletionResult]::new('--username', '--username', [CompletionResultType]::ParameterName, 'Local username. Prompted when omitted')
            [CompletionResult]::new('--address', '--address', [CompletionResultType]::ParameterName, 'Temporal gRPC address override when the auth service does not advertise one')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'Namespace stored in a newly created profile')
            [CompletionResult]::new('--namespace', '--namespace', [CompletionResultType]::ParameterName, 'Namespace stored in a newly created profile')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('--password-stdin', '--password-stdin', [CompletionResultType]::ParameterName, 'Read the password from stdin instead of a terminal prompt')
            [CompletionResult]::new('--allow-http', '--allow-http', [CompletionResultType]::ParameterName, 'Permit loopback-only HTTP for local development and tests')
            [CompletionResult]::new('--set-default', '--set-default', [CompletionResultType]::ParameterName, 'Make the selected profile the default')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;auth;whoami' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;auth;logout' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;auth;help' {
            [CompletionResult]::new('login', 'login', [CompletionResultType]::ParameterValue, 'Sign in with a local username and a password read without echo')
            [CompletionResult]::new('whoami', 'whoami', [CompletionResultType]::ParameterValue, 'Show the current signed-in identity and session status')
            [CompletionResult]::new('logout', 'logout', [CompletionResultType]::ParameterValue, 'Revoke the refresh grant and remove its local credential')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'temporal-tui;auth;help;login' {
            break
        }
        'temporal-tui;auth;help;whoami' {
            break
        }
        'temporal-tui;auth;help;logout' {
            break
        }
        'temporal-tui;auth;help;help' {
            break
        }
        'temporal-tui;config-path' {
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Alternate config file. Defaults to the platform user config directory')
            [CompletionResult]::new('--profile', '--profile', [CompletionResultType]::ParameterName, 'Named connection profile')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'temporal-tui;help' {
            [CompletionResult]::new('profile', 'profile', [CompletionResultType]::ParameterValue, 'Manage connection profiles')
            [CompletionResult]::new('filter', 'filter', [CompletionResultType]::ParameterValue, 'Manage saved visibility queries')
            [CompletionResult]::new('auth', 'auth', [CompletionResultType]::ParameterValue, 'Sign in to a protected self-hosted Temporal deployment')
            [CompletionResult]::new('config-path', 'config-path', [CompletionResultType]::ParameterValue, 'Print the active config path')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'temporal-tui;help;profile' {
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List configured profiles without resolving secrets')
            [CompletionResult]::new('show', 'show', [CompletionResultType]::ParameterValue, 'Print one redacted profile as TOML')
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create a connection profile')
            [CompletionResult]::new('set-default', 'set-default', [CompletionResultType]::ParameterValue, 'Select the default profile')
            [CompletionResult]::new('set-api-key', 'set-api-key', [CompletionResultType]::ParameterValue, 'Read an API key without echo and store it in the OS credential manager')
            [CompletionResult]::new('clear-api-key', 'clear-api-key', [CompletionResultType]::ParameterValue, 'Remove an API key reference and delete its OS credential')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a profile')
            break
        }
        'temporal-tui;help;profile;list' {
            break
        }
        'temporal-tui;help;profile;show' {
            break
        }
        'temporal-tui;help;profile;create' {
            break
        }
        'temporal-tui;help;profile;set-default' {
            break
        }
        'temporal-tui;help;profile;set-api-key' {
            break
        }
        'temporal-tui;help;profile;clear-api-key' {
            break
        }
        'temporal-tui;help;profile;remove' {
            break
        }
        'temporal-tui;help;filter' {
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List saved visibility queries')
            [CompletionResult]::new('save', 'save', [CompletionResultType]::ParameterValue, 'Save or replace a visibility query')
            [CompletionResult]::new('remove', 'remove', [CompletionResultType]::ParameterValue, 'Remove a saved visibility query')
            break
        }
        'temporal-tui;help;filter;list' {
            break
        }
        'temporal-tui;help;filter;save' {
            break
        }
        'temporal-tui;help;filter;remove' {
            break
        }
        'temporal-tui;help;auth' {
            [CompletionResult]::new('login', 'login', [CompletionResultType]::ParameterValue, 'Sign in with a local username and a password read without echo')
            [CompletionResult]::new('whoami', 'whoami', [CompletionResultType]::ParameterValue, 'Show the current signed-in identity and session status')
            [CompletionResult]::new('logout', 'logout', [CompletionResultType]::ParameterValue, 'Revoke the refresh grant and remove its local credential')
            break
        }
        'temporal-tui;help;auth;login' {
            break
        }
        'temporal-tui;help;auth;whoami' {
            break
        }
        'temporal-tui;help;auth;logout' {
            break
        }
        'temporal-tui;help;config-path' {
            break
        }
        'temporal-tui;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
