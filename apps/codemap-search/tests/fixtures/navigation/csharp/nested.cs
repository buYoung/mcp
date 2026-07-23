using Repo = Demo.Repository;
public class Service
{
    public void Run(Mapper mapper, Repo repo)
    {
        var dto = mapper.Map();
        repo.Save(dto);
    }
}

