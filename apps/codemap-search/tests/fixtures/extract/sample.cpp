/** Fixture exercising C++ branch-sensitive extraction. */

/** A widget with a public and a private member. */
class Widget {
public:
    /** Draw the widget. */
    void draw();

private:
    int hiddenHelper();
};

/** Out-of-line definition: owner is Widget. */
void Widget::draw() {
    const char *label = "widget label";
    (void)label;
}

int Widget::hiddenHelper() {
    return 0;
}

/** An exported free function. */
const char *makeWidget() {
    return "widget";
}

/**
 * @deprecated use makeWidget instead
 */
const char *makeWidgetLegacy() {
    return "legacy";
}

/* A file-local helper (static is not exported). */
static int internalCount() {
    return 0;
}
