using Rule = Demo.Rule;
public class Policy
{
    public bool Allow(Rule rule)
    {
        var passed = rule.Check();
        return passed;
    }
}

