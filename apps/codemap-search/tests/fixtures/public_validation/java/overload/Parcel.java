package overload;

public class Parcel extends Envelope {
    public String seal(Object cover) { return "cover"; }

    public String weigh(int grams) { return "grams"; }
}
