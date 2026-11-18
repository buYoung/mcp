namespace Twin;

public interface IDrain
{
    void Flush();
}

public class DrainBase
{
    public void Flush() {}
}

public partial class Pump : DrainBase, IDrain
{
}
