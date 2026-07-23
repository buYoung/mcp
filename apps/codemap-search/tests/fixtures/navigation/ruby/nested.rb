require_relative "repo"
def run(mapper, repo)
  dto = mapper.map()
  repo.save(dto)
end

