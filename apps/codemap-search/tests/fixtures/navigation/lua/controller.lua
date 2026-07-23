local request_module = require("./request")
function handle(service)
  local request_value = request.parse()
  service.submit(request_value)
end

