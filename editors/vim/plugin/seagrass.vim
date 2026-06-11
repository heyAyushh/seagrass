" Vim package entrypoint for Seagrass.
" Requires vim-lsp for the LSP transport; Seagrass remains the language server.

if exists('g:loaded_seagrass_vim')
  finish
endif
let g:loaded_seagrass_vim = 1

let s:server_name = 'seagrass'
let s:default_root_markers = ['Anchor.toml', 'Seagrass.toml', 'Cargo.toml']

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
        \ 'inlayHints.enabled': v:true,
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
  echo 'root markers: ' . join(s:Setting('seagrass_root_markers', s:default_root_markers), ', ')
  echo 'transport: vim-lsp'
endfunction

command! SeagrassInfo call s:Info()

augroup seagrass_vim_lsp
  autocmd!
  autocmd User lsp_setup call <SID>RegisterServer()
augroup END
