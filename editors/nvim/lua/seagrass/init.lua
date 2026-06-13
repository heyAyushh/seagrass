local M = {}

local SERVER_NAME = "seagrass"
local DEFAULT_COMMAND = "seagrass"
local DEFAULT_ROOT_MARKERS = { "Anchor.toml", "Seagrass.toml", "Cargo.toml" }
local LSP_TO_EDITOR_INDEX_OFFSET = 1
local DIAGNOSTICS_SUCCESS_EXIT_CODE = 0
local DIAGNOSTICS_FINDINGS_EXIT_CODE = 1

local SEAGRASS_COMMANDS = {
  status = "seagrass/status",
  analyze = "seagrass/analyze",
  artifacts = "seagrass/artifacts",
  program_report = "seagrass/programReport",
  error_coverage = "seagrass/errorCoverage",
  support_matrix = "seagrass/supportMatrix",
  generator_profile = "seagrass/generatorProfile",
  logs = "seagrass/logs",
  project_coverage = "seagrass/projectCoverage",
  feedback = "seagrass/feedback",
}

local DEFAULT_SETTINGS = {
  ["agent.mode"] = false,
  ["diagnostics.security.enabled"] = true,
  ["diagnostics.experimental.enabled"] = true,
  ["security.strictNative.enabled"] = true,
  ["diagnostics.transport"] = "push",
  ["diagnostics.coldPath"] = "idle",
  ["editor.client"] = "nvim",
  ["editor.inlineValues.enabled"] = false,
  ["telemetry.completion.enabled"] = true,
  ["telemetry.diagnostics.enabled"] = true,
  ["workspaceIndex.enabled"] = true,
  ["trace.server"] = false,
}

local state = {
  options = {},
  commands_created = false,
}

local function merge(left, right)
  return vim.tbl_deep_extend("force", left or {}, right or {})
end

local function setting_value(settings, key)
  local value = settings[key]
  if value ~= nil then
    return value
  end
  return DEFAULT_SETTINGS[key]
end

local function notify(message, level)
  vim.notify(message, level or vim.log.levels.INFO, { title = "Seagrass" })
end

local function json_encode(value)
  if vim.json and vim.json.encode then
    return vim.json.encode(value)
  end
  return vim.fn.json_encode(value)
end

local function json_decode(text)
  if vim.json and vim.json.decode then
    return vim.json.decode(text)
  end
  return vim.fn.json_decode(text)
end

local function executable(command)
  return vim.fn.executable(command) == 1
end

local function path_join(left, right)
  return left .. "/" .. right
end

local function parent_dir(path)
  return vim.fn.fnamemodify(path, ":h")
end

local function file_exists(path)
  return vim.fn.filereadable(path) == 1 or vim.fn.isdirectory(path) == 1
end

local function root_dir_for(markers, start_path)
  local path = start_path ~= "" and start_path or vim.fn.getcwd()
  local dir = vim.fn.isdirectory(path) == 1 and path or parent_dir(path)

  while dir ~= "" do
    for _, marker in ipairs(markers) do
      if file_exists(path_join(dir, marker)) then
        return dir
      end
    end

    local parent = parent_dir(dir)
    if parent == dir then
      break
    end
    dir = parent
  end

  return vim.fn.getcwd()
end

local function current_uri()
  local name = vim.api.nvim_buf_get_name(0)
  if name == "" or vim.bo.filetype ~= "rust" then
    return nil
  end
  return vim.uri_from_fname(name)
end

local function parse_document_arguments(raw_args, include_current_document)
  local argument = {}
  if include_current_document then
    local uri = current_uri()
    if uri then
      argument.uri = uri
    end
  end

  for token in string.gmatch(raw_args or "", "%S+") do
    local key, value = token:match("^([^=]+)=(.*)$")
    if key == "instruction" or key == "function" or key == "context" then
      argument[key] = value
    elseif key == "uri" then
      argument.uri = value
    elseif key == "path" then
      argument.uri = vim.uri_from_fname(vim.fn.fnamemodify(value, ":p"))
    elseif token ~= "" then
      argument.uri = vim.uri_from_fname(vim.fn.fnamemodify(token, ":p"))
    end
  end

  if next(argument) == nil then
    return {}
  end
  return { argument }
end

local function active_seagrass_client()
  local bufnr = vim.api.nvim_get_current_buf()
  local get_clients = vim.lsp.get_clients or vim.lsp.get_active_clients
  local clients = get_clients({ name = SERVER_NAME, bufnr = bufnr })
  if #clients == 0 then
    clients = get_clients({ name = SERVER_NAME })
  end
  return clients[1]
end

local function open_output(title, value)
  local lines
  if type(value) == "string" then
    lines = vim.split(value, "\n", { plain = true })
  else
    lines = vim.split(json_encode(value), "\n", { plain = true })
  end

  vim.cmd("botright new")
  local bufnr = vim.api.nvim_get_current_buf()
  vim.api.nvim_buf_set_name(bufnr, title)
  vim.bo[bufnr].buftype = "nofile"
  vim.bo[bufnr].bufhidden = "wipe"
  vim.bo[bufnr].swapfile = false
  vim.bo[bufnr].filetype = "json"
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  vim.api.nvim_win_set_cursor(0, { 1, 0 })
end

function M.execute(command, arguments, title)
  local client = active_seagrass_client()
  if not client then
    notify("Start the Seagrass language server before running " .. command .. ".", vim.log.levels.WARN)
    return
  end

  client.request("workspace/executeCommand", {
    command = command,
    arguments = arguments or {},
  }, function(error, result)
    if error then
      notify("Seagrass command failed: " .. tostring(error.message or error), vim.log.levels.ERROR)
      return
    end
    open_output(title or command, result)
  end, vim.api.nvim_get_current_buf())
end

local function execute_document_command(command, raw_args, title)
  M.execute(command, parse_document_arguments(raw_args, true), title)
end

local function cli_command(arguments)
  local options = state.options or {}
  local command = options.cli_command or options.command or DEFAULT_COMMAND
  local args = vim.deepcopy(options.cli_args or {})
  vim.list_extend(args, arguments)
  return command, args
end

local function diagnostics_target(raw_args)
  if raw_args and raw_args ~= "" then
    return raw_args
  end
  local name = vim.api.nvim_buf_get_name(0)
  if vim.bo.filetype == "rust" and name ~= "" then
    return name
  end
  return root_dir_for(state.options.root_markers or DEFAULT_ROOT_MARKERS, vim.fn.getcwd())
end

local function run_cli(arguments)
  local command, args = cli_command(arguments)
  if vim.system then
    local result = vim.system(vim.list_extend({ command }, args), { text = true }):wait()
    return result.code, result.stdout or "", result.stderr or ""
  end
  local output = vim.fn.systemlist(vim.list_extend({ command }, args))
  return vim.v.shell_error, table.concat(output, "\n"), ""
end

local function diagnostic_type(severity)
  if severity == "ERROR" then
    return "E"
  end
  if severity == "WARNING" then
    return "W"
  end
  return "I"
end

local function diagnostic_text(diagnostic)
  local prefix = diagnostic.code or diagnostic.topic
  if prefix and prefix ~= "" then
    return prefix .. ": " .. (diagnostic.message or "")
  end
  return diagnostic.message or ""
end

local function quickfix_items(diagnostics)
  local items = {}
  for _, diagnostic in ipairs(diagnostics) do
    local start = (((diagnostic or {}).range or {}).start or {})
    table.insert(items, {
      filename = diagnostic.file,
      lnum = (start.line or 0) + LSP_TO_EDITOR_INDEX_OFFSET,
      col = (start.character or 0) + LSP_TO_EDITOR_INDEX_OFFSET,
      type = diagnostic_type(diagnostic.severity),
      text = diagnostic_text(diagnostic),
    })
  end
  return items
end

function M.diagnostics(raw_args)
  local target = diagnostics_target(raw_args)
  local exit_code, stdout, stderr = run_cli({ "diagnostics", target, "--json" })
  if exit_code ~= DIAGNOSTICS_SUCCESS_EXIT_CODE and exit_code ~= DIAGNOSTICS_FINDINGS_EXIT_CODE then
    open_output("Seagrass diagnostics error", stderr ~= "" and stderr or stdout)
    return
  end

  local ok, diagnostics = pcall(json_decode, stdout)
  if not ok then
    open_output("Seagrass diagnostics parse error", stdout)
    return
  end

  local items = quickfix_items(diagnostics)
  vim.fn.setqflist({}, "r", { title = "Seagrass diagnostics", items = items })
  if #items == 0 then
    vim.cmd("cclose")
    notify("No diagnostics.")
  else
    vim.cmd("copen")
  end
end

function M.analyze_cli(raw_args)
  local target = diagnostics_target(raw_args)
  local exit_code, stdout, stderr = run_cli({ "analyze", target, "--json" })
  if exit_code ~= DIAGNOSTICS_SUCCESS_EXIT_CODE then
    open_output("Seagrass analysis error", stderr ~= "" and stderr or stdout)
    return
  end
  open_output("Seagrass analysis", stdout)
end

function M.restart()
  local get_clients = vim.lsp.get_clients or vim.lsp.get_active_clients
  for _, client in ipairs(get_clients({ name = SERVER_NAME })) do
    client.stop(true)
  end
  if vim.lsp.enable then
    vim.lsp.enable(SERVER_NAME, false)
    vim.lsp.enable(SERVER_NAME, true)
  else
    notify("Stopped Seagrass clients. Reopen a Rust buffer to start it again.")
  end
end

local function create_commands()
  if state.commands_created then
    return
  end
  state.commands_created = true

  vim.api.nvim_create_user_command("SeagrassInfo", function()
    local command, args = cli_command({})
    notify(
      "server: " .. table.concat(vim.list_extend({ state.options.command or DEFAULT_COMMAND }, state.options.args or {}), " ")
        .. "\ncli: " .. table.concat(vim.list_extend({ command }, args), " ")
        .. "\nroot markers: " .. table.concat(state.options.root_markers or DEFAULT_ROOT_MARKERS, ", ")
        .. "\ntransport: native Neovim LSP"
    )
  end, {})

  vim.api.nvim_create_user_command("SeagrassStatus", function()
    M.execute(SEAGRASS_COMMANDS.status, {}, "Seagrass status")
  end, {})
  vim.api.nvim_create_user_command("SeagrassAnalyze", function(opts)
    execute_document_command(SEAGRASS_COMMANDS.analyze, opts.args, "Seagrass document analysis")
  end, { nargs = "*", complete = "file" })
  vim.api.nvim_create_user_command("SeagrassArtifacts", function(opts)
    execute_document_command(SEAGRASS_COMMANDS.artifacts, opts.args, "Seagrass artifacts")
  end, { nargs = "*", complete = "file" })
  vim.api.nvim_create_user_command("SeagrassProgramReport", function()
    M.execute(SEAGRASS_COMMANDS.program_report, {}, "Seagrass program report")
  end, {})
  vim.api.nvim_create_user_command("SeagrassErrorCoverage", function()
    M.execute(SEAGRASS_COMMANDS.error_coverage, {}, "Seagrass error coverage")
  end, {})
  vim.api.nvim_create_user_command("SeagrassSupportMatrix", function()
    M.execute(SEAGRASS_COMMANDS.support_matrix, {}, "Seagrass support matrix")
  end, {})
  vim.api.nvim_create_user_command("SeagrassGeneratorProfile", function()
    M.execute(SEAGRASS_COMMANDS.generator_profile, {}, "Seagrass generator profile")
  end, {})
  vim.api.nvim_create_user_command("SeagrassLogs", function()
    M.execute(SEAGRASS_COMMANDS.logs, {}, "Seagrass recent logs")
  end, {})
  vim.api.nvim_create_user_command("SeagrassProjectCoverage", function()
    M.execute(SEAGRASS_COMMANDS.project_coverage, {}, "Seagrass project coverage")
  end, {})
  vim.api.nvim_create_user_command("SeagrassFeedback", function()
    M.execute(SEAGRASS_COMMANDS.feedback, {}, "Seagrass feedback")
  end, {})
  vim.api.nvim_create_user_command("SeagrassDiagnostics", function(opts)
    M.diagnostics(opts.args)
  end, { nargs = "?", complete = "file" })
  vim.api.nvim_create_user_command("SeagrassAnalyzeCli", function(opts)
    M.analyze_cli(opts.args)
  end, { nargs = "?", complete = "file" })
  vim.api.nvim_create_user_command("SeagrassRestart", M.restart, {})
end

local function lsp_config(options)
  local command = options.command or DEFAULT_COMMAND
  local args = vim.deepcopy(options.args or {})
  local root_markers = options.root_markers or DEFAULT_ROOT_MARKERS
  local settings = options.settings or {}
  return {
    cmd = vim.list_extend({ command }, args),
    filetypes = { "rust" },
    root_dir = function(fname)
      return root_dir_for(root_markers, fname)
    end,
    settings = {
      seagrass = merge(DEFAULT_SETTINGS, options.settings or {}),
    },
    init_options = {
      seagrass = {
        agent = {
          mode = setting_value(settings, "agent.mode"),
        },
        diagnostics = {
          transport = setting_value(settings, "diagnostics.transport"),
        },
        editor = {
          client = "nvim",
          inlineValues = {
            enabled = setting_value(settings, "editor.inlineValues.enabled"),
          },
        },
        telemetry = {
          completion = {
            enabled = setting_value(settings, "telemetry.completion.enabled"),
          },
          diagnostics = {
            enabled = setting_value(settings, "telemetry.diagnostics.enabled"),
          },
        },
      },
    },
  }
end

local function setup_builtin_lsp(config)
  if type(vim.lsp.config) ~= "function" or type(vim.lsp.enable) ~= "function" then
    return false
  end
  vim.lsp.config(SERVER_NAME, config)
  vim.lsp.enable(SERVER_NAME)
  return true
end

local function setup_lspconfig(config)
  local ok, lspconfig = pcall(require, "lspconfig")
  if not ok then
    notify("Install nvim-lspconfig or Neovim 0.11+ built-in vim.lsp.config to auto-register Seagrass.", vim.log.levels.WARN)
    return
  end

  local configs = require("lspconfig.configs")
  if not configs[SERVER_NAME] then
    configs[SERVER_NAME] = {
      default_config = config,
    }
  end
  lspconfig[SERVER_NAME].setup(config)
end

function M.setup(options)
  local resolved = merge({
    command = DEFAULT_COMMAND,
    args = {},
    root_markers = DEFAULT_ROOT_MARKERS,
    settings = {},
    auto_start = true,
    commands = true,
  }, options or {})
  state.options = resolved

  if resolved.commands ~= false then
    create_commands()
  end

  if resolved.auto_start == false then
    return
  end

  if not executable(resolved.command) then
    notify("`" .. resolved.command .. "` is not executable; set require('seagrass').setup({ command = ... }).", vim.log.levels.WARN)
    return
  end

  local config = lsp_config(resolved)
  if not setup_builtin_lsp(config) then
    setup_lspconfig(config)
  end
end

return M
