_temporal-tui() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="temporal__tui"
                ;;
            temporal__tui,auth)
                cmd="temporal__tui__subcmd__auth"
                ;;
            temporal__tui,config-path)
                cmd="temporal__tui__subcmd__config__subcmd__path"
                ;;
            temporal__tui,filter)
                cmd="temporal__tui__subcmd__filter"
                ;;
            temporal__tui,help)
                cmd="temporal__tui__subcmd__help"
                ;;
            temporal__tui,profile)
                cmd="temporal__tui__subcmd__profile"
                ;;
            temporal__tui__subcmd__auth,help)
                cmd="temporal__tui__subcmd__auth__subcmd__help"
                ;;
            temporal__tui__subcmd__auth,login)
                cmd="temporal__tui__subcmd__auth__subcmd__login"
                ;;
            temporal__tui__subcmd__auth,logout)
                cmd="temporal__tui__subcmd__auth__subcmd__logout"
                ;;
            temporal__tui__subcmd__auth,whoami)
                cmd="temporal__tui__subcmd__auth__subcmd__whoami"
                ;;
            temporal__tui__subcmd__auth__subcmd__help,help)
                cmd="temporal__tui__subcmd__auth__subcmd__help__subcmd__help"
                ;;
            temporal__tui__subcmd__auth__subcmd__help,login)
                cmd="temporal__tui__subcmd__auth__subcmd__help__subcmd__login"
                ;;
            temporal__tui__subcmd__auth__subcmd__help,logout)
                cmd="temporal__tui__subcmd__auth__subcmd__help__subcmd__logout"
                ;;
            temporal__tui__subcmd__auth__subcmd__help,whoami)
                cmd="temporal__tui__subcmd__auth__subcmd__help__subcmd__whoami"
                ;;
            temporal__tui__subcmd__filter,help)
                cmd="temporal__tui__subcmd__filter__subcmd__help"
                ;;
            temporal__tui__subcmd__filter,list)
                cmd="temporal__tui__subcmd__filter__subcmd__list"
                ;;
            temporal__tui__subcmd__filter,remove)
                cmd="temporal__tui__subcmd__filter__subcmd__remove"
                ;;
            temporal__tui__subcmd__filter,save)
                cmd="temporal__tui__subcmd__filter__subcmd__save"
                ;;
            temporal__tui__subcmd__filter__subcmd__help,help)
                cmd="temporal__tui__subcmd__filter__subcmd__help__subcmd__help"
                ;;
            temporal__tui__subcmd__filter__subcmd__help,list)
                cmd="temporal__tui__subcmd__filter__subcmd__help__subcmd__list"
                ;;
            temporal__tui__subcmd__filter__subcmd__help,remove)
                cmd="temporal__tui__subcmd__filter__subcmd__help__subcmd__remove"
                ;;
            temporal__tui__subcmd__filter__subcmd__help,save)
                cmd="temporal__tui__subcmd__filter__subcmd__help__subcmd__save"
                ;;
            temporal__tui__subcmd__help,auth)
                cmd="temporal__tui__subcmd__help__subcmd__auth"
                ;;
            temporal__tui__subcmd__help,config-path)
                cmd="temporal__tui__subcmd__help__subcmd__config__subcmd__path"
                ;;
            temporal__tui__subcmd__help,filter)
                cmd="temporal__tui__subcmd__help__subcmd__filter"
                ;;
            temporal__tui__subcmd__help,help)
                cmd="temporal__tui__subcmd__help__subcmd__help"
                ;;
            temporal__tui__subcmd__help,profile)
                cmd="temporal__tui__subcmd__help__subcmd__profile"
                ;;
            temporal__tui__subcmd__help__subcmd__auth,login)
                cmd="temporal__tui__subcmd__help__subcmd__auth__subcmd__login"
                ;;
            temporal__tui__subcmd__help__subcmd__auth,logout)
                cmd="temporal__tui__subcmd__help__subcmd__auth__subcmd__logout"
                ;;
            temporal__tui__subcmd__help__subcmd__auth,whoami)
                cmd="temporal__tui__subcmd__help__subcmd__auth__subcmd__whoami"
                ;;
            temporal__tui__subcmd__help__subcmd__filter,list)
                cmd="temporal__tui__subcmd__help__subcmd__filter__subcmd__list"
                ;;
            temporal__tui__subcmd__help__subcmd__filter,remove)
                cmd="temporal__tui__subcmd__help__subcmd__filter__subcmd__remove"
                ;;
            temporal__tui__subcmd__help__subcmd__filter,save)
                cmd="temporal__tui__subcmd__help__subcmd__filter__subcmd__save"
                ;;
            temporal__tui__subcmd__help__subcmd__profile,clear-api-key)
                cmd="temporal__tui__subcmd__help__subcmd__profile__subcmd__clear__subcmd__api__subcmd__key"
                ;;
            temporal__tui__subcmd__help__subcmd__profile,create)
                cmd="temporal__tui__subcmd__help__subcmd__profile__subcmd__create"
                ;;
            temporal__tui__subcmd__help__subcmd__profile,list)
                cmd="temporal__tui__subcmd__help__subcmd__profile__subcmd__list"
                ;;
            temporal__tui__subcmd__help__subcmd__profile,remove)
                cmd="temporal__tui__subcmd__help__subcmd__profile__subcmd__remove"
                ;;
            temporal__tui__subcmd__help__subcmd__profile,set-api-key)
                cmd="temporal__tui__subcmd__help__subcmd__profile__subcmd__set__subcmd__api__subcmd__key"
                ;;
            temporal__tui__subcmd__help__subcmd__profile,set-default)
                cmd="temporal__tui__subcmd__help__subcmd__profile__subcmd__set__subcmd__default"
                ;;
            temporal__tui__subcmd__help__subcmd__profile,show)
                cmd="temporal__tui__subcmd__help__subcmd__profile__subcmd__show"
                ;;
            temporal__tui__subcmd__profile,clear-api-key)
                cmd="temporal__tui__subcmd__profile__subcmd__clear__subcmd__api__subcmd__key"
                ;;
            temporal__tui__subcmd__profile,create)
                cmd="temporal__tui__subcmd__profile__subcmd__create"
                ;;
            temporal__tui__subcmd__profile,help)
                cmd="temporal__tui__subcmd__profile__subcmd__help"
                ;;
            temporal__tui__subcmd__profile,list)
                cmd="temporal__tui__subcmd__profile__subcmd__list"
                ;;
            temporal__tui__subcmd__profile,remove)
                cmd="temporal__tui__subcmd__profile__subcmd__remove"
                ;;
            temporal__tui__subcmd__profile,set-api-key)
                cmd="temporal__tui__subcmd__profile__subcmd__set__subcmd__api__subcmd__key"
                ;;
            temporal__tui__subcmd__profile,set-default)
                cmd="temporal__tui__subcmd__profile__subcmd__set__subcmd__default"
                ;;
            temporal__tui__subcmd__profile,show)
                cmd="temporal__tui__subcmd__profile__subcmd__show"
                ;;
            temporal__tui__subcmd__profile__subcmd__help,clear-api-key)
                cmd="temporal__tui__subcmd__profile__subcmd__help__subcmd__clear__subcmd__api__subcmd__key"
                ;;
            temporal__tui__subcmd__profile__subcmd__help,create)
                cmd="temporal__tui__subcmd__profile__subcmd__help__subcmd__create"
                ;;
            temporal__tui__subcmd__profile__subcmd__help,help)
                cmd="temporal__tui__subcmd__profile__subcmd__help__subcmd__help"
                ;;
            temporal__tui__subcmd__profile__subcmd__help,list)
                cmd="temporal__tui__subcmd__profile__subcmd__help__subcmd__list"
                ;;
            temporal__tui__subcmd__profile__subcmd__help,remove)
                cmd="temporal__tui__subcmd__profile__subcmd__help__subcmd__remove"
                ;;
            temporal__tui__subcmd__profile__subcmd__help,set-api-key)
                cmd="temporal__tui__subcmd__profile__subcmd__help__subcmd__set__subcmd__api__subcmd__key"
                ;;
            temporal__tui__subcmd__profile__subcmd__help,set-default)
                cmd="temporal__tui__subcmd__profile__subcmd__help__subcmd__set__subcmd__default"
                ;;
            temporal__tui__subcmd__profile__subcmd__help,show)
                cmd="temporal__tui__subcmd__profile__subcmd__help__subcmd__show"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        temporal__tui)
            opts="-n -q -h -V --config --profile --command-timeout --address --namespace --api-key --tls --tls-ca --tls-cert --tls-key --tls-server-name --header --codec-endpoint --codec-header --codec-auth --query --page-size --refresh-seconds --no-auto-refresh --no-color --read-only --web-ui-url --help --version profile filter auth config-path help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --api-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-ca)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-cert)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-server-name)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --header)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --codec-endpoint)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --codec-header)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --codec-auth)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --query)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -q)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --page-size)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --refresh-seconds)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --web-ui-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__auth)
            opts="-h --config --profile --command-timeout --help login whoami logout help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__auth__subcmd__help)
            opts="login whoami logout help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__auth__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__auth__subcmd__help__subcmd__login)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__auth__subcmd__help__subcmd__logout)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__auth__subcmd__help__subcmd__whoami)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__auth__subcmd__login)
            opts="-n -h --url --username --address --namespace --password-stdin --allow-http --set-default --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --username)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__auth__subcmd__logout)
            opts="-h --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__auth__subcmd__whoami)
            opts="-h --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__config__subcmd__path)
            opts="-h --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__filter)
            opts="-h --config --profile --command-timeout --help list save remove help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__filter__subcmd__help)
            opts="list save remove help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__filter__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__filter__subcmd__help__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__filter__subcmd__help__subcmd__remove)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__filter__subcmd__help__subcmd__save)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__filter__subcmd__list)
            opts="-h --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__filter__subcmd__remove)
            opts="-h --yes --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__filter__subcmd__save)
            opts="-h --replace --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help)
            opts="profile filter auth config-path help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__auth)
            opts="login whoami logout"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__auth__subcmd__login)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__auth__subcmd__logout)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__auth__subcmd__whoami)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__config__subcmd__path)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__filter)
            opts="list save remove"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__filter__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__filter__subcmd__remove)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__filter__subcmd__save)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__profile)
            opts="list show create set-default set-api-key clear-api-key remove"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__profile__subcmd__clear__subcmd__api__subcmd__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__profile__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__profile__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__profile__subcmd__remove)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__profile__subcmd__set__subcmd__api__subcmd__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__profile__subcmd__set__subcmd__default)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__help__subcmd__profile__subcmd__show)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile)
            opts="-h --config --profile --command-timeout --help list show create set-default set-api-key clear-api-key remove help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__clear__subcmd__api__subcmd__key)
            opts="-h --yes --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__create)
            opts="-n -h --address --namespace --tls --tls-ca --tls-cert --tls-key --tls-server-name --header --codec-endpoint --codec-header --codec-auth-env --api-key-env --web-ui-url --read-only --set-default --replace --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --namespace)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-ca)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-cert)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tls-server-name)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --header)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --codec-endpoint)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --codec-header)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --codec-auth-env)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --api-key-env)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --web-ui-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__help)
            opts="list show create set-default set-api-key clear-api-key remove help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__help__subcmd__clear__subcmd__api__subcmd__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__help__subcmd__create)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__help__subcmd__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__help__subcmd__remove)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__help__subcmd__set__subcmd__api__subcmd__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__help__subcmd__set__subcmd__default)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__help__subcmd__show)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__list)
            opts="-h --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__remove)
            opts="-h --yes --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__set__subcmd__api__subcmd__key)
            opts="-h --from-env --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --from-env)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__set__subcmd__default)
            opts="-h --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        temporal__subcmd__tui__subcmd__profile__subcmd__show)
            opts="-h --config --profile --command-timeout --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --command-timeout)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _temporal-tui -o nosort -o bashdefault -o default temporal-tui
else
    complete -F _temporal-tui -o bashdefault -o default temporal-tui
fi
