import demo.Helper

class Worker {
    String name = 'groovy'

    @Deprecated
    void run() {
        Helper.start()
    }
}
