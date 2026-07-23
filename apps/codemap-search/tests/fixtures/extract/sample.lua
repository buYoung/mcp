local worker = require("./worker")
local state = "ready"
local service = {
  status = "idle",
  run = function()
    worker.execute()
  end,
}

function service.start(clock)
  clock:tick()
  return worker.new()
end

function service:stop()
  self.status = "stopped"
end

local function helper()
  return dofile("./helper.lua")
end

