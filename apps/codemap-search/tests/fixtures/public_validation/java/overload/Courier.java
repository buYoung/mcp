package overload;

public class Courier {
    // Java selects the inherited Envelope.seal(String), whose parameter fits the argument best.
    public String send(Parcel parcel) {
        return parcel.seal("fragile");
    }

    // Only Parcel declares weigh, so the call reaches it.
    public String scale(Parcel parcel) {
        return parcel.weigh(3);
    }

    public static void main(String[] args) {
        System.out.println(new Courier().send(new Parcel()) + " " + new Courier().scale(new Parcel()));
    }
}
