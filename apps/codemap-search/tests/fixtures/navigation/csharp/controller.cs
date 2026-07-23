using Request = Demo.Request;
public class Controller
{
    public void Handle(Service service)
    {
        var request = Request.Parse();
        service.Submit(request);
    }
}

