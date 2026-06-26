import json

from checkout.parser import parse_order


def handle(request, service, response):
    payload = parse_order(json.loads(request.body))
    result = service.submit(payload)
    return response.json(result)
