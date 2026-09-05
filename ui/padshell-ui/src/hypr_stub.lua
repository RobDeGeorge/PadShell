-- Evaluates a Hyprland Lua config under a fake `hl` API that records
-- every hl.bind() call instead of doing anything, so padshell can list
-- the binds without needing Hyprland's interpreter. Run as:
--   lua - <config.lua>   (script on stdin, config path as arg[1])
-- Output: one TSV line per bind:
--   line \t submap \t keys \t dispatcher \t args \t flags
-- Tabs/newlines inside fields are replaced with spaces.

local config_path = arg and arg[1]
if not config_path then
  io.stderr:write("usage: lua - <hyprland.lua>\n")
  os.exit(2)
end

local binds = {}
local current_submap = ""

-- A descriptor is what hl.dsp.<path>(args) returns: {path=..., args=...}.
local function make_dsp(path)
  return setmetatable({}, {
    __index = function(_, k) return make_dsp(path == "" and k or (path .. "." .. k)) end,
    __call = function(_, ...) return { __dsp = true, path = path, args = { ... } } end,
  })
end

local function str(v)
  if v == nil then return "" end
  return tostring(v)
end

-- Translate a descriptor into the hyprlang (dispatcher, args) pair the
-- rest of the parser already understands.
local function translate(d)
  local p, a = d.path, d.args[1]
  local function tbl(k) return type(a) == "table" and a[k] or nil end
  if p == "exec_cmd" or p == "exec" then return "exec", str(a) end
  if p == "window.close" then return "killactive", "" end
  if p == "window.fullscreen" then return "fullscreen", str(tbl("mode")) end
  if p == "window.float" then return "togglefloating", "" end
  if p == "window.pin" then return "pin", "" end
  if p == "window.drag" then return "movewindow", "" end
  if p == "window.move" then
    if tbl("direction") then return "movewindow", str(tbl("direction")) end
    if tbl("workspace") then
      return (tbl("silent") and "movetoworkspacesilent" or "movetoworkspace"), str(tbl("workspace"))
    end
    return "movewindow", ""
  end
  if p == "window.resize" then
    if type(a) == "table" and (a.x or a.y) then return "resizeactive", str(a.x or 0) .. " " .. str(a.y or 0) end
    return "resizewindow", ""
  end
  if p == "window.swap" then return "swapwindow", str(tbl("direction")) end
  if p == "focus" then
    if tbl("direction") then return "movefocus", str(tbl("direction")) end
    if tbl("workspace") then return "workspace", str(tbl("workspace")) end
    if tbl("window") then return "focuswindow", "" end
    if tbl("urgent") then return "focusurgentorlast", "" end
    return "focus", ""
  end
  if p == "workspace.toggle_special" then return "togglespecialworkspace", str(a) end
  if p == "workspace.focus" or p == "workspace" then return "workspace", str(a) end
  if p == "layout" then return "layoutmsg", str(a) end
  if p == "exit" then return "exit", "" end
  if p == "group.toggle" then return "togglegroup", "" end
  if p == "group.next" then return "changegroupactive", "f" end
  if p == "group.prev" then return "changegroupactive", "b" end
  if p == "group.move_window" then
    if tbl("action") == "out" then return "moveoutofgroup", "" end
    return "moveintogroup", str(tbl("direction"))
  end
  if p == "submap" then return "submap", str(a) end
  if p == "reload" then return "forcerendererreload", "" end
  -- Fallback: last path segment as dispatcher, first arg stringified.
  local disp = p:match("([^.]+)$") or p
  if type(a) == "table" then
    local parts = {}
    for k, v in pairs(a) do parts[#parts + 1] = str(k) .. "=" .. str(v) end
    table.sort(parts)
    return disp, table.concat(parts, " ")
  end
  return disp, str(a)
end

local function clean(s)
  return (str(s):gsub("[\t\r\n]", " "))
end

local hl = {}
hl.dsp = make_dsp("")

function hl.bind(keys, action, opts)
  local info = debug.getinfo(2, "l")
  local line = info and info.currentline or 0
  local dispatcher, args
  if type(action) == "function" then
    dispatcher, args = "__lua", ""
  elseif type(action) == "table" and action.__dsp then
    dispatcher, args = translate(action)
  else
    dispatcher, args = "__unknown", str(action)
  end
  local flags = ""
  if type(opts) == "table" then
    if opts.mouse then flags = flags .. "m" end
    if opts.repeating then flags = flags .. "e" end
    if opts.locked then flags = flags .. "l" end
    if opts.release then flags = flags .. "r" end
  end
  binds[#binds + 1] = table.concat({
    str(line), clean(current_submap), clean(keys), clean(dispatcher), clean(args), flags,
  }, "\t")
end

function hl.define_submap(name, fn)
  local prev = current_submap
  current_submap = str(name)
  if type(fn) == "function" then pcall(fn) end
  current_submap = prev
end

-- Everything else on hl is a harmless no-op that returns nil-safe values.
setmetatable(hl, {
  __index = function(_, k)
    return function(...) return nil end
  end,
})

-- Let `require("colors")` etc. resolve relative to the config file, but
-- swallow failures - theme files aren't needed to list binds.
local dir = config_path:match("^(.*)/[^/]*$") or "."
package.path = dir .. "/?.lua;" .. package.path
local real_require = require
require = function(name)
  local ok, mod = pcall(real_require, name)
  if ok then return mod end
  return {}
end

_G.hl = hl
local chunk, err = loadfile(config_path)
if not chunk then
  io.stderr:write("padshell: cannot load " .. config_path .. ": " .. tostring(err) .. "\n")
  os.exit(1)
end
local ok, run_err = pcall(chunk)
if not ok then
  io.stderr:write("padshell: error while evaluating config: " .. tostring(run_err) .. "\n")
end
for _, line in ipairs(binds) do print(line) end
