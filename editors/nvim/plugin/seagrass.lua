if vim.g.loaded_seagrass_nvim == 1 then
  return
end
vim.g.loaded_seagrass_nvim = 1

local ok, seagrass = pcall(require, "seagrass")
if not ok then
  vim.notify("Seagrass: failed to load Neovim package.", vim.log.levels.ERROR)
  return
end

local user_options = vim.g.seagrass_nvim or {}
seagrass.setup(user_options)
