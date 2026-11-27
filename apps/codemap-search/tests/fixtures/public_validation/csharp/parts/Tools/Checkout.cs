namespace Parts;

// A class of the same name in another project: not a part of the class in Shop.
public partial class Checkout
{
    public void Reset()
    {
        Verify(0);
    }

    private void Verify(decimal amount)
    {
    }
}
