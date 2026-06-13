" Vim package entrypoint for Seagrass.
" Requires vim-lsp for the LSP transport; Seagrass remains the language server.

if exists('g:loaded_seagrass_vim')
  finish
endif
let g:loaded_seagrass_vim = 1

let s:server_name = 'seagrass'
let s:default_root_markers = ['Anchor.toml', 'Seagrass.toml', 'Cargo.toml']
let s:lsp_index_offset = 1
let s:diagnostics_success_exit_code = 0
let s:diagnostics_findings_exit_code = 1
let s:seagrass_commands = {
      \ 'status': 'seagrass/status',
      \ 'analyze': 'seagrass/analyze',
      \ 'artifacts': 'seagrass/artifacts',
      \ 'program_report': 'seagrass/programReport',
      \ 'error_coverage': 'seagrass/errorCoverage',
      \ 'support_matrix': 'seagrass/supportMatrix',
      \ 'generator_profile': 'seagrass/generatorProfile',
      \ 'logs': 'seagrass/logs',
      \ 'project_coverage': 'seagrass/projectCoverage',
      \ 'feedback': 'seagrass/feedback',
      \ }

function! s:Setting(name, default) abort
  return get(g:, a:name, a:default)
endfunction

function! s:ServerCommand(_server_info) abort
  let l:command = s:Setting('seagrass_command', 'seagrass')
  let l:args = s:Setting('seagrass_args', [])
  return [l:command] + l:args
endfunction

function! s:WorkspaceSettings() abort
  let l:settings = {
        \ 'agent.mode': v:false,
        \ 'diagnostics.security.enabled': v:true,
        \ 'diagnostics.experimental.enabled': v:true,
        \ 'security.strictNative.enabled': v:true,
        \ 'diagnostics.transport': 'push',
        \ 'diagnostics.coldPath': 'idle',
        \ 'editor.client': 'vim',
        \ 'editor.inlineValues.enabled': v:false,
        \ 'telemetry.completion.enabled': v:true,
        \ 'telemetry.diagnostics.enabled': v:true,
        \ 'workspaceIndex.enabled': v:true,
        \ 'trace.server': v:false,
        \ }
  return extend(l:settings, s:Setting('seagrass_settings', {}), 'force')
endfunction

function! s:InitializationOptions() abort
  let l:settings = s:WorkspaceSettings()
  return {
        \ 'seagrass': {
        \   'agent': {
        \     'mode': get(l:settings, 'agent.mode', v:false),
        \   },
        \   'diagnostics': {
        \     'transport': get(l:settings, 'diagnostics.transport', 'push'),
        \   },
        \   'editor': {
        \     'client': 'vim',
        \     'inlineValues': {
        \       'enabled': get(l:settings, 'editor.inlineValues.enabled', v:false),
        \     },
        \   },
        \   'telemetry': {
        \     'completion': {
        \       'enabled': get(l:settings, 'telemetry.completion.enabled', v:true),
        \     },
        \     'diagnostics': {
        \       'enabled': get(l:settings, 'telemetry.diagnostics.enabled', v:true),
        \     },
        \   },
        \ },
        \ }
endfunction

function! s:RootUri(_server_info) abort
  return s:PathToUri(s:FindRoot(expand('%:p')))
endfunction

function! s:FindRoot(buffer_path) abort
  let l:markers = s:Setting('seagrass_root_markers', s:default_root_markers)
  let l:dir = empty(a:buffer_path)
        \ ? getcwd()
        \ : fnamemodify(a:buffer_path, ':p:h')

  while !empty(l:dir)
    for l:marker in l:markers
      if filereadable(l:dir . '/' . l:marker) || isdirectory(l:dir . '/' . l:marker)
        return l:dir
      endif
    endfor

    let l:parent = fnamemodify(l:dir, ':h')
    if l:parent ==# l:dir
      break
    endif
    let l:dir = l:parent
  endwhile

  return getcwd()
endfunction

function! s:PathToUri(path) abort
  if exists('*lsp#utils#path_to_uri')
    return lsp#utils#path_to_uri(a:path)
  endif
  let l:path = substitute(fnamemodify(a:path, ':p'), '\\', '/', 'g')
  return 'file://' . l:path
endfunction

function! s:CurrentDocumentArgument() abort
  if expand('%:e') !=# 'rs' || empty(expand('%:p'))
    return {}
  endif
  return {'uri': s:PathToUri(expand('%:p'))}
endfunction

function! s:DocumentCommandArguments(raw_args) abort
  let l:argument = s:CurrentDocumentArgument()
  for l:token in split(a:raw_args)
    let l:match = matchlist(l:token, '^\([^=]\+\)=\(.*\)$')
    if !empty(l:match)
      let l:key = l:match[1]
      let l:value = l:match[2]
      if index(['instruction', 'function', 'context'], l:key) >= 0
        let l:argument[l:key] = l:value
      elseif l:key ==# 'uri'
        let l:argument.uri = l:value
      elseif l:key ==# 'path'
        let l:argument.uri = s:PathToUri(fnamemodify(l:value, ':p'))
      endif
    elseif !empty(l:token)
      let l:argument.uri = s:PathToUri(fnamemodify(l:token, ':p'))
    endif
  endfor
  return empty(l:argument) ? [] : [l:argument]
endfunction

function! s:Warn(message) abort
  echohl WarningMsg
  echom a:message
  echohl None
endfunction

function! s:OpenScratch(title, lines) abort
  botright new
  execute 'file ' . fnameescape(a:title)
  setlocal buftype=nofile bufhidden=wipe noswapfile nobuflisted
  setlocal filetype=json
  call setline(1, empty(a:lines) ? [''] : a:lines)
  normal! gg
endfunction

function! s:JsonLines(value) abort
  if type(a:value) == type('')
    return split(a:value, "\n", v:true)
  endif
  return [json_encode(a:value)]
endfunction

function! s:HandleExecuteResponse(title, data) abort
  if exists('*lsp#client#is_error') && lsp#client#is_error(a:data)
    call s:Warn('Seagrass command failed: ' . string(a:data))
    return
  endif
  let l:response = get(a:data, 'response', {})
  if !has_key(l:response, 'result')
    call s:Warn('Seagrass command returned no result.')
    return
  endif
  call s:OpenScratch(a:title, s:JsonLines(l:response.result))
endfunction

function! s:ExecuteCommand(command, arguments, title) abort
  if !exists('*lsp#send_request')
    call s:Warn('Seagrass: install vim-lsp before using LSP-backed Seagrass commands.')
    return
  endif
  call lsp#send_request(s:server_name, {
        \ 'method': 'workspace/executeCommand',
        \ 'params': {
        \   'command': a:command,
        \   'arguments': a:arguments,
        \ },
        \ 'on_notification': function('s:HandleExecuteResponse', [a:title]),
        \ 'bufnr': bufnr('%'),
        \ })
endfunction

function! s:ExecuteDocumentCommand(command, raw_args, title) abort
  call s:ExecuteCommand(a:command, s:DocumentCommandArguments(a:raw_args), a:title)
endfunction

function! s:CliCommand(arguments) abort
  let l:command = s:Setting('seagrass_cli_command', s:Setting('seagrass_command', 'seagrass'))
  let l:args = s:Setting('seagrass_cli_args', [])
  return [l:command] + l:args + a:arguments
endfunction

function! s:ShellCommand(arguments) abort
  return join(map(copy(s:CliCommand(a:arguments)), 'shellescape(v:val)'), ' ')
endfunction

function! s:DiagnosticsTarget(raw_args) abort
  if !empty(a:raw_args)
    return a:raw_args
  endif
  if expand('%:e') ==# 'rs' && !empty(expand('%:p'))
    return expand('%:p')
  endif
  return s:FindRoot(expand('%:p'))
endfunction

function! s:RunCli(arguments) abort
  let l:output = systemlist(s:ShellCommand(a:arguments))
  return {'exit_code': v:shell_error, 'output': l:output}
endfunction

function! s:DiagnosticType(severity) abort
  if a:severity ==# 'ERROR'
    return 'E'
  endif
  if a:severity ==# 'WARNING'
    return 'W'
  endif
  return 'I'
endfunction

function! s:DiagnosticText(diagnostic) abort
  let l:code = get(a:diagnostic, 'code', '')
  let l:topic = get(a:diagnostic, 'topic', '')
  let l:prefix = empty(l:code) ? l:topic : l:code
  return empty(l:prefix)
        \ ? get(a:diagnostic, 'message', '')
        \ : l:prefix . ': ' . get(a:diagnostic, 'message', '')
endfunction

function! s:QuickfixItems(diagnostics) abort
  let l:items = []
  for l:diagnostic in a:diagnostics
    let l:range = get(l:diagnostic, 'range', {})
    let l:start = get(l:range, 'start', {})
    call add(l:items, {
          \ 'filename': get(l:diagnostic, 'file', ''),
          \ 'lnum': get(l:start, 'line', 0) + s:lsp_index_offset,
          \ 'col': get(l:start, 'character', 0) + s:lsp_index_offset,
          \ 'type': s:DiagnosticType(get(l:diagnostic, 'severity', '')),
          \ 'text': s:DiagnosticText(l:diagnostic),
          \ })
  endfor
  return l:items
endfunction

function! s:DiagnosticsQuickfix(raw_args) abort
  let l:target = s:DiagnosticsTarget(a:raw_args)
  let l:result = s:RunCli(['diagnostics', l:target, '--json'])
  if index([s:diagnostics_success_exit_code, s:diagnostics_findings_exit_code], l:result.exit_code) < 0
    call s:OpenScratch('Seagrass diagnostics error', l:result.output)
    return
  endif
  try
    let l:diagnostics = json_decode(join(l:result.output, "\n"))
  catch
    call s:OpenScratch('Seagrass diagnostics parse error', l:result.output)
    return
  endtry
  let l:items = s:QuickfixItems(l:diagnostics)
  call setqflist([], 'r', {'title': 'Seagrass diagnostics', 'items': l:items})
  if empty(l:items)
    cclose
    echom 'Seagrass: no diagnostics.'
  else
    copen
  endif
endfunction

function! s:AnalyzeCli(raw_args) abort
  let l:target = s:DiagnosticsTarget(a:raw_args)
  let l:result = s:RunCli(['analyze', l:target, '--json'])
  if l:result.exit_code != s:diagnostics_success_exit_code
    call s:OpenScratch('Seagrass analysis error', l:result.output)
    return
  endif
  call s:OpenScratch('Seagrass analysis', l:result.output)
endfunction

function! s:Restart() abort
  if exists(':LspRestartServer') == 2
    execute 'LspRestartServer ' . s:server_name
  elseif exists(':LspStopServer') == 2 && exists(':LspStartServer') == 2
    execute 'LspStopServer ' . s:server_name
    execute 'LspStartServer ' . s:server_name
  else
    call s:Warn('Seagrass: this vim-lsp build does not expose a stable restart command; reopen the Rust buffer after changing server launch settings.')
  endif
endfunction

function! s:RegisterServer() abort
  if !exists('*lsp#register_server')
    return
  endif

  let l:command = s:Setting('seagrass_command', 'seagrass')
  if !executable(l:command)
    echohl WarningMsg
    echom 'Seagrass: "' . l:command . '" is not executable; set g:seagrass_command or install the seagrass binary.'
    echohl None
    return
  endif

  let l:config = {
        \ 'name': s:server_name,
        \ 'cmd': function('s:ServerCommand'),
        \ 'root_uri': function('s:RootUri'),
        \ 'allowlist': ['rust'],
        \ 'whitelist': ['rust'],
        \ 'initialization_options': s:InitializationOptions(),
        \ 'workspace_config': {
        \   'seagrass': s:WorkspaceSettings(),
        \ },
        \ }
  call lsp#register_server(l:config)
endfunction

function! s:Info() abort
  echo 'Seagrass Vim package'
  echo 'server: ' . join(s:ServerCommand({}), ' ')
  echo 'cli: ' . join(s:CliCommand([]), ' ')
  echo 'root markers: ' . join(s:Setting('seagrass_root_markers', s:default_root_markers), ', ')
  echo 'transport: vim-lsp'
endfunction

command! SeagrassInfo call s:Info()
command! SeagrassStatus call s:ExecuteCommand(s:seagrass_commands.status, [], 'Seagrass status')
command! -nargs=* -complete=file SeagrassAnalyze call s:ExecuteDocumentCommand(s:seagrass_commands.analyze, <q-args>, 'Seagrass document analysis')
command! -nargs=* -complete=file SeagrassArtifacts call s:ExecuteDocumentCommand(s:seagrass_commands.artifacts, <q-args>, 'Seagrass artifacts')
command! SeagrassProgramReport call s:ExecuteCommand(s:seagrass_commands.program_report, [], 'Seagrass program report')
command! SeagrassErrorCoverage call s:ExecuteCommand(s:seagrass_commands.error_coverage, [], 'Seagrass error coverage')
command! SeagrassSupportMatrix call s:ExecuteCommand(s:seagrass_commands.support_matrix, [], 'Seagrass support matrix')
command! SeagrassGeneratorProfile call s:ExecuteCommand(s:seagrass_commands.generator_profile, [], 'Seagrass generator profile')
command! SeagrassLogs call s:ExecuteCommand(s:seagrass_commands.logs, [], 'Seagrass recent logs')
command! SeagrassProjectCoverage call s:ExecuteCommand(s:seagrass_commands.project_coverage, [], 'Seagrass project coverage')
command! SeagrassFeedback call s:ExecuteCommand(s:seagrass_commands.feedback, [], 'Seagrass feedback')
command! -nargs=? -complete=file SeagrassDiagnostics call s:DiagnosticsQuickfix(<q-args>)
command! -nargs=? -complete=file SeagrassAnalyzeCli call s:AnalyzeCli(<q-args>)
command! SeagrassRestart call s:Restart()

augroup seagrass_vim_lsp
  autocmd!
  autocmd User lsp_setup call <SID>RegisterServer()
augroup END
