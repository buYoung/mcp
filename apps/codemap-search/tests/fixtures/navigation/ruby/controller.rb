require_relative "request"
def handle(service)
  request = Request.parse()
  service.submit(request)
end

