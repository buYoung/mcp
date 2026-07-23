local repo_module = require("./repo")
function run(mapper, repo)
  local dto = mapper.map()
  repo.save(dto)
end

